import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, expect, it, vi } from "vitest";
import { SettingsApplyStatus } from "./settings-apply-status";
const ipc = vi.hoisted(() => ({
  getSettingsApplyStatus: vi.fn(),
  applyPendingSettings: vi.fn(),
}));
vi.mock("@/ipc/commands", () => ipc);
beforeEach(() => {
  vi.resetAllMocks();
});
function mount(saving = false) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={client}>
      <SettingsApplyStatus saving={saving} failed={false} />
    </QueryClientProvider>,
  );
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
  mount();
  const apply = await screen.findByRole("button", {
    name: "Apply and reconnect",
  });
  expect(ipc.applyPendingSettings).not.toHaveBeenCalled();
  await userEvent.click(apply);
  await screen.findByText("Connection settings are up to date.");
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
it("shows next-connection behavior when disconnected and prevents apply while saving", async () => {
  ipc.getSettingsApplyStatus.mockResolvedValue({
    connected: false,
    action: "none",
  });
  const first = mount();
  await screen.findByText(
    "Connection settings take effect on the next connection.",
  );
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
