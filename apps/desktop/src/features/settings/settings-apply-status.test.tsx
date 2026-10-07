import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createTestQueryClient, renderWithQuery } from "@voya/features/test/render";
import { beforeEach, expect, it, vi } from "vite-plus/test";
import { queryKeys } from "@voya/client/query-keys";
import { SettingsApplyStatus } from "./settings-apply-status";
import { useSettingsApplyStatus } from "@voya/features/settings/use-settings-apply-status";
import { installFakeCommands } from "@voya/features/test/backend";
const ipc = installFakeCommands({
  getSettingsApplyStatus: vi.fn(),
  applyPendingSettings: vi.fn(),
});
beforeEach(() => {
  vi.resetAllMocks();
});
function mount(saving = false) {
  const client = createTestQueryClient({ gcTime: 0 });
  const view = renderWithQuery(<SettingsApplyStatus saving={saving} failed={false} />, { queryClient: client });
  return { ...view, client };
}
it("keeps saved settings pending until the explicit reconnect action", async () => {
  ipc.getSettingsApplyStatus.mockResolvedValue({
    connected: true,
    action: "reconnect",
  });
  ipc.applyPendingSettings.mockImplementation(async () => {
    ipc.getSettingsApplyStatus.mockResolvedValue({
      connected: true,
      action: "none",
    });
  });
  const { container } = mount();
  const apply = await screen.findByRole("button", {
    name: "Apply and reconnect",
  });
  expect(ipc.applyPendingSettings).not.toHaveBeenCalled();
  await userEvent.click(apply);
  await waitFor(() => expect(container).toBeEmptyDOMElement());
  expect(ipc.applyPendingSettings).toHaveBeenCalledOnce();
});
it("retains the pending proxy action after failure and retries it", async () => {
  ipc.getSettingsApplyStatus.mockResolvedValue({
    connected: true,
    action: "reapplyProxy",
  });
  ipc.applyPendingSettings
    .mockRejectedValueOnce(new Error("proxy failed"))
    .mockResolvedValueOnce({ connected: true, action: "reapplyProxy" });
  mount();
  await userEvent.click(
    await screen.findByRole("button", { name: "Apply proxy settings" }),
  );
  expect(await screen.findByRole("alert")).toHaveTextContent("proxy failed");
  await userEvent.click(screen.getByRole("button", { name: "Retry" }));
  await waitFor(() =>
    expect(ipc.applyPendingSettings).toHaveBeenCalledTimes(2),
  );
  expect(
    await screen.findByRole("button", { name: "Apply proxy settings" }),
  ).toBeEnabled();
});
it("drops a failed apply's error once nothing is left to apply", async () => {
  ipc.getSettingsApplyStatus.mockResolvedValue({ connected: true, action: "reapplyProxy" });
  ipc.applyPendingSettings.mockRejectedValue(new Error("proxy failed"));
  const { client, container } = mount();
  await userEvent.click(await screen.findByRole("button", { name: "Apply proxy settings" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("proxy failed");

  // The core reconnected from elsewhere: there is nothing to retry any more.
  ipc.getSettingsApplyStatus.mockResolvedValue({ connected: true, action: "none" });
  await client.refetchQueries({ queryKey: queryKeys.settingsApply });

  await waitFor(() => expect(container).toBeEmptyDOMElement());
});
it("hides the banner when disconnected and prevents apply while saving", async () => {
  ipc.getSettingsApplyStatus.mockResolvedValue({
    connected: false,
    action: "none",
  });
  const first = mount();
  await waitFor(() =>
    expect(first.client.getQueryState(queryKeys.settingsApply)?.status).toBe(
      "success",
    ),
  );
  expect(first.container).toBeEmptyDOMElement();
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
  first.unmount();
  ipc.getSettingsApplyStatus.mockResolvedValue({
    connected: true,
    action: "reconnect",
  });
  mount(true);
  expect(
    await screen.findByRole("button", { name: "Apply and reconnect" }),
  ).toBeDisabled();
});
it("says when changes are being saved and once they are saved", async () => {
  ipc.getSettingsApplyStatus.mockResolvedValue({
    connected: false,
    action: "none",
  });
  const status = (props: { saving: boolean; saved?: boolean; failed?: boolean }) => (
    <SettingsApplyStatus failed={props.failed ?? false} saved={props.saved} saving={props.saving} />
  );
  const view = renderWithQuery(status({ saving: true }), { queryClient: createTestQueryClient({ gcTime: 0 }) });
  expect(await screen.findByRole("status")).toHaveTextContent("Saving…");
  view.rerender(status({ saved: true, saving: false }));
  expect(
    await screen.findByText(
      "All changes saved. Connection changes take effect the next time you connect.",
    ),
  ).toBeInTheDocument();
  view.rerender(status({ failed: true, saved: true, saving: false }));
  expect(view.container).toBeEmptyDOMElement();
});
it("asks once on mount, and again when a save finishes", async () => {
  ipc.getSettingsApplyStatus.mockResolvedValue({
    connected: true,
    action: "none",
  });
  const client = createTestQueryClient({ gcTime: 0 });
  const view = renderWithQuery(<SettingsApplyStatus saving={false} failed={false} />, {
    queryClient: client,
  });
  await waitFor(() => expect(client.isFetching()).toBe(0));
  expect(ipc.getSettingsApplyStatus).toHaveBeenCalledOnce();

  // While a save runs the answer would describe the settings before it.
  view.rerender(<SettingsApplyStatus saving failed={false} />);
  await waitFor(() => expect(client.isFetching()).toBe(0));
  expect(ipc.getSettingsApplyStatus).toHaveBeenCalledOnce();

  view.rerender(<SettingsApplyStatus saving={false} failed={false} />);
  await waitFor(() => expect(ipc.getSettingsApplyStatus).toHaveBeenCalledTimes(2));
});
it("asks once when a caller enables it as its connection appears", async () => {
  // Home's summary: disabled while disconnected, keyed by the connection.
  function Summary({ connection }: { connection: string | null }) {
    useSettingsApplyStatus({ enabled: connection !== null, refreshKey: connection });
    return null;
  }
  ipc.getSettingsApplyStatus.mockResolvedValue({ connected: true, action: "none" });
  const client = createTestQueryClient({ gcTime: 0 });
  const view = renderWithQuery(<Summary connection={null} />, { queryClient: client });
  expect(ipc.getSettingsApplyStatus).not.toHaveBeenCalled();

  view.rerender(<Summary connection="tokyo::1" />);
  await waitFor(() => expect(client.isFetching()).toBe(0));
  expect(ipc.getSettingsApplyStatus).toHaveBeenCalledOnce();

  // The next connection is a new answer even while the last one is fresh.
  view.rerender(<Summary connection="tokyo::2" />);
  await waitFor(() => expect(ipc.getSettingsApplyStatus).toHaveBeenCalledTimes(2));
});
