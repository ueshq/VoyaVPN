import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AppDnsSettings, AppearanceSettings } from "@/ipc/bindings";

import { makeAppSettings } from "./app-settings.test-fixture";
import { useAppSettings } from "./use-app-settings";

const ipcMocks = vi.hoisted(() => ({
  loadAppSettings: vi.fn(),
  saveAppSettings: vi.fn(),
}));
const preferenceMocks = vi.hoisted(() => ({
  applyUiPreferences: vi.fn((preferences: AppearanceSettings, options?: { persist?: boolean }) => {
    void preferences;
    void options;
    return Promise.resolve();
  }),
}));

vi.mock("@/ipc", () => ipcMocks);
vi.mock("@/features/settings/ui-preferences", () => ({
  applyUiPreferences: preferenceMocks.applyUiPreferences,
  UI_PREFERENCES_QUERY_KEY: ["ui-preferences"],
}));

describe("useAppSettings", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    window.localStorage.clear();
    ipcMocks.loadAppSettings.mockResolvedValue(makeAppSettings());
    ipcMocks.saveAppSettings.mockImplementation(async (settings) => settings);
  });

  afterEach(cleanup);

  it("saves cross-section edits as one authoritative bundle", async () => {
    const user = userEvent.setup();
    renderProbe();
    await screen.findByText("clean");

    await user.click(screen.getByRole("button", { name: "Edit two sections" }));
    expect(screen.getByTestId("state")).toHaveTextContent("dirty");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(ipcMocks.saveAppSettings).toHaveBeenCalledTimes(1));
    expect(ipcMocks.saveAppSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        sources: expect.objectContaining({ subscriptionConverter: "https://convert.example.test" }),
        core: expect.objectContaining({ logLevel: "debug" }),
      }),
    );
    await waitFor(() => expect(screen.getByTestId("state")).toHaveTextContent("clean"));
  });

  it("previews an appearance without persisting it and restores the original on discard", async () => {
    const user = userEvent.setup();
    renderProbe();
    await screen.findByText("clean");

    await user.click(screen.getByRole("button", { name: "Preview dark" }));
    // `persist: false` is what keeps an unsaved edit out of localStorage; the
    // controller must never fall back to snapshotting the storage keys itself.
    await waitFor(() =>
      expect(preferenceMocks.applyUiPreferences).toHaveBeenCalledWith(
        { language: "en", theme: "dark" },
        { persist: false },
      ),
    );
    await user.click(screen.getByRole("button", { name: "Discard" }));

    await waitFor(() =>
      expect(preferenceMocks.applyUiPreferences).toHaveBeenLastCalledWith({ language: "en", theme: "system" }),
    );
    expect(screen.getByTestId("theme")).toHaveTextContent("system");
    expect(screen.getByTestId("state")).toHaveTextContent("clean");
  });

  it("posts the freshest DNS block even when the draft was seeded before the DNS pane saved", async () => {
    const user = userEvent.setup();
    const { client } = renderProbe();
    await screen.findByText("clean");

    // Seed the whole-bundle draft first (dns = the snapshot loaded on open)…
    await user.click(screen.getByRole("button", { name: "Edit two sections" }));
    // …then let the DNS pane write its own section through its own command.
    client.setQueryData(["dns"], freshDns());
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(ipcMocks.saveAppSettings).toHaveBeenCalledTimes(1));
    expect(ipcMocks.saveAppSettings).toHaveBeenCalledWith(
      expect.objectContaining({ dns: freshDns() }),
    );
  });

  it("keeps the draft's own DNS block when the DNS pane was never opened", async () => {
    const user = userEvent.setup();
    renderProbe();
    await screen.findByText("clean");

    await user.click(screen.getByRole("button", { name: "Edit two sections" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(ipcMocks.saveAppSettings).toHaveBeenCalledTimes(1));
    expect(ipcMocks.saveAppSettings).toHaveBeenCalledWith(
      expect.objectContaining({ dns: makeAppSettings().dns }),
    );
  });

  it("reloads the authoritative snapshot after a failed save", async () => {
    const user = userEvent.setup();
    const authoritative = makeAppSettings({ subscriptionConverter: "https://authoritative.example.test" });
    ipcMocks.loadAppSettings
      .mockResolvedValueOnce(makeAppSettings())
      .mockResolvedValueOnce(authoritative);
    ipcMocks.saveAppSettings.mockRejectedValue(new Error("save failed"));
    renderProbe();
    await screen.findByText("clean");

    await user.click(screen.getByRole("button", { name: "Edit two sections" }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("save failed");
    await waitFor(() =>
      expect(screen.getByTestId("converter")).toHaveTextContent("https://authoritative.example.test"),
    );
    expect(screen.getByTestId("state")).toHaveTextContent("clean");
  });

  it("reports reload and appearance-preview failures", async () => {
    const user = userEvent.setup();
    ipcMocks.loadAppSettings
      .mockResolvedValueOnce(makeAppSettings())
      .mockRejectedValueOnce(new Error("reload failed"));
    preferenceMocks.applyUiPreferences.mockRejectedValueOnce(new Error("preview failed"));
    renderProbe();
    await screen.findByText("clean");

    await user.click(screen.getByRole("button", { name: "Preview dark" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("preview failed");
    await user.click(screen.getByRole("button", { name: "Reload" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("reload failed");
  });

  it("keeps the original save error when recovery loading also fails", async () => {
    const user = userEvent.setup();
    ipcMocks.loadAppSettings
      .mockResolvedValueOnce(makeAppSettings())
      .mockRejectedValueOnce(new Error("recovery unavailable"));
    ipcMocks.saveAppSettings.mockRejectedValueOnce(new Error("save rejected"));
    renderProbe();
    await screen.findByText("clean");

    await user.click(screen.getByRole("button", { name: "Edit two sections" }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("save rejected");
  });

  it("returns safely when save and discard run before the initial snapshot", async () => {
    const user = userEvent.setup();
    ipcMocks.loadAppSettings.mockReturnValue(new Promise(() => {}));
    renderBareProbe();

    await user.click(screen.getByRole("button", { name: "Early save" }));
    await user.click(screen.getByRole("button", { name: "Early discard" }));
    expect(ipcMocks.saveAppSettings).not.toHaveBeenCalled();
  });
});

function Probe() {
  const controller = useAppSettings();
  if (!controller.settings) return <div>loading</div>;
  return (
    <div>
      <div data-testid="state">{controller.dirty ? "dirty" : "clean"}</div>
      <div data-testid="theme">{controller.settings.appearance.theme}</div>
      <div data-testid="converter">{controller.settings.sources.subscriptionConverter ?? "none"}</div>
      {controller.error ? <div role="alert">{controller.error}</div> : null}
      <button
        onClick={() =>
          controller.update((settings) => ({
            ...settings,
            sources: {
              ...settings.sources,
              subscriptionConverter: "https://convert.example.test",
            },
            core: { ...settings.core, logLevel: "debug" },
          }))
        }
        type="button"
      >
        Edit two sections
      </button>
      <button
        onClick={() =>
          controller.setAppearance({ ...controller.settings!.appearance, theme: "dark" })
        }
        type="button"
      >
        Preview dark
      </button>
      <button onClick={() => void controller.discard()} type="button">Discard</button>
      <button onClick={() => void controller.save()} type="button">Save</button>
      <button onClick={() => void controller.reload()} type="button">Reload</button>
    </div>
  );
}

function BareProbe() {
  const controller = useAppSettings();
  return (
    <div>
      <button onClick={() => void controller.save()} type="button">Early save</button>
      <button onClick={() => void controller.discard()} type="button">Early discard</button>
    </div>
  );
}

function renderBareProbe() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <BareProbe />
    </QueryClientProvider>,
  );
}

function renderProbe() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return {
    client,
    ...render(
      <QueryClientProvider client={client}>
        <Probe />
      </QueryClientProvider>,
    ),
  };
}

function freshDns(): AppDnsSettings {
  return { ...makeAppSettings().dns, remote: "https://dns.example.test/dns-query" };
}
