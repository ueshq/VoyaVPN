import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { changeLocale } from "@voya/i18n";
import type { AppSettingsV1 } from "@/ipc/bindings";
import { useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";
import {
  deferred,
  resetSettingsBackend,
  serverSettings,
  settingsIpc,
} from "./settings-backend.test-fixture";
import { SettingsScreen } from "./settings-screen";
import { settingsSaveQueue } from "./settings-save-queue";

vi.mock(
  "@/ipc/commands",
  async () => (await import("./settings-backend.test-fixture")).settingsIpc,
);
vi.mock("@/ipc/updater", () => ({
  check: vi.fn(),
  getVersion: vi.fn(async () => "0.1.0"),
}));
vi.mock("@/ipc/process", () => ({ relaunch: vi.fn() }));
beforeEach(async () => {
  resetSettingsBackend();
  await changeLocale("en");
  useShellStore.setState({ activeTab: "settings", settingsTab: "general" });
  useToastStore.setState({ toasts: [] });
});
afterEach(cleanup);
function mount() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  function Host() {
    const tab = useShellStore((s) => s.activeTab);
    return (
      <>
        <button
          onClick={() => useShellStore.getState().setActiveTab("profiles")}
        >
          Leave
        </button>
        <button
          onClick={() => useShellStore.getState().setActiveTab("settings")}
        >
          Return
        </button>
        {tab === "settings" ? <SettingsScreen /> : <p>Other page</p>}
      </>
    );
  }
  return {
    ...render(
      <QueryClientProvider client={client}>
        <Host />
      </QueryClientProvider>,
    ),
    settle: () => act(() => settingsSaveQueue(client).settled()),
  };
}

describe("redesigned automatic settings", () => {
  it("opens an unvisited Advanced tab at the running log and consumes the focus request", async () => {
    const scroll = vi.spyOn(Element.prototype, "scrollIntoView");
    mount();
    await screen.findByLabelText("Autostart");
    act(() => useShellStore.getState().openSettings("advanced", "logs"));
    const heading = await screen.findByRole("heading", { name: "Runtime logs" });
    await waitFor(() => expect(heading).toHaveFocus());
    expect(scroll).toHaveBeenCalledWith({ block: "start" });
    expect(heading.closest("section")).toHaveAttribute("data-settings-highlight", "true");
    expect(useShellStore.getState().settingsTarget).toBeNull();
    scroll.mockRestore();
  });

  it("uses four compact categories, accessible groups, and no manual save actions", async () => {
    mount();
    await screen.findByLabelText("Autostart");
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      "General",
      "Connection",
      "Advanced",
      "Updates",
    ]);
    expect(
      screen.getByRole("heading", { level: 1, name: "Settings" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("region", { name: "Appearance" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Save all|Discard changes|Reload/ }),
    ).not.toBeInTheDocument();
    expect(settingsIpc.loadDnsSettings).not.toHaveBeenCalled();
    const user = userEvent.setup();
    for (const name of ["Connection", "Advanced", "Updates"])
      await user.click(screen.getByRole("tab", { name }));
    expect(
      await screen.findByRole("heading", { name: "Rule library" }),
    ).toBeVisible();
  });

  it("submits switches immediately and text on Enter without resubmitting on blur", async () => {
    const { settle } = mount();
    fireEvent.click(await screen.findByLabelText("Autostart"));
    await settle();
    expect(serverSettings().behavior.autostart).toBe(true);
    await userEvent.click(screen.getByRole("tab", { name: "Advanced" }));
    const input = await screen.findByLabelText("User-Agent");
    fireEvent.change(input, { target: { value: "new-agent" } });
    expect(serverSettings().core.defaultUserAgent).toBe("agent-before-edit");
    fireEvent.keyDown(input, { key: "Enter" });
    await settle();
    fireEvent.blur(input);
    await settle();
    expect(serverSettings().core.defaultUserAgent).toBe("new-agent");
    expect(settingsIpc.saveAppSettings).toHaveBeenCalledTimes(2);
  });

  it("preserves local text across categories and commits on category change", async () => {
    const user = userEvent.setup();
    const { settle } = mount();
    await user.click(screen.getByRole("tab", { name: "Advanced" }));
    const input = await screen.findByLabelText("User-Agent");
    await user.clear(input);
    await user.type(input, "new-agent");
    await user.click(screen.getByRole("tab", { name: "Connection" }));
    await settle();
    await user.click(screen.getByRole("tab", { name: "Advanced" }));
    expect(input).toHaveValue("new-agent");
    expect(serverSettings().core.defaultUserAgent).toBe("new-agent");
  });

  it("retains a failed input and retries from the single error banner", async () => {
    const { settle } = mount();
    await userEvent.click(screen.getByRole("tab", { name: "Advanced" }));
    const input = await screen.findByLabelText("User-Agent");
    settingsIpc.saveAppSettings.mockRejectedValueOnce(
      new Error("write failed"),
    );
    fireEvent.change(input, { target: { value: "retry-agent" } });
    fireEvent.blur(input);
    await settle();
    expect(input).toHaveValue("retry-agent");
    expect(screen.getByRole("alert")).toHaveTextContent("write failed");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await settle();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(serverSettings().core.defaultUserAgent).toBe("retry-agent");
  });

  it("flushes focused input on imperative navigation and never opens a leave dialog", async () => {
    const { settle } = mount();
    await userEvent.click(screen.getByRole("tab", { name: "Advanced" }));
    const input = await screen.findByLabelText("User-Agent");
    fireEvent.change(input, { target: { value: "on-leave" } });
    act(() => useShellStore.getState().setActiveTab("profiles"));
    expect(screen.getByText("Other page")).toBeVisible();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await settle();
    expect(serverSettings().core.defaultUserAgent).toBe("on-leave");
  });

  it("allows leaving during a save, reports failure, and retains the rejected draft for retry", async () => {
    const { settle } = mount();
    await userEvent.click(screen.getByRole("tab", { name: "Advanced" }));
    const input = await screen.findByLabelText("User-Agent");
    const pending = deferred<AppSettingsV1>();
    settingsIpc.saveAppSettings.mockReturnValueOnce(pending.promise);
    fireEvent.change(input, { target: { value: "failed-agent" } });
    fireEvent.blur(input);
    await waitFor(() =>
      expect(settingsIpc.saveAppSettings).toHaveBeenCalledTimes(1),
    );
    expect(input).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Leave" }));
    expect(screen.getByText("Other page")).toBeVisible();
    pending.reject(new Error("write failed after leaving"));
    await settle();
    expect(useToastStore.getState().toasts.at(-1)?.description).toBe(
      "write failed after leaving",
    );
    fireEvent.click(screen.getByRole("button", { name: "Return" }));
    await userEvent.click(screen.getByRole("tab", { name: "Advanced" }));
    expect(await screen.findByLabelText("User-Agent")).toHaveValue(
      "failed-agent",
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "write failed after leaving",
    );
  });

  it("saves DNS without adding a manual action or dropping its edit on exit", async () => {
    const { settle } = mount();
    await screen.findByLabelText("Autostart");
    await userEvent.click(screen.getByRole("tab", { name: "Connection" }));
    const resolver = await screen.findByLabelText("Remote DNS");
    fireEvent.change(resolver, { target: { value: "1.1.1.1" } });
    act(() => useShellStore.getState().setActiveTab("profiles"));
    await settle();
    expect(serverSettings().dns.remote).toBe("1.1.1.1");
  });
});
