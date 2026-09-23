import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { saveQueue } from "@voya/features/forms/save-queue";
import { UpdatesPanel } from "@/features/updates/updates-panel";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { changeLocale } from "@voya/i18n";
import type { AppUpdaterStatus, ResourceUpdateFile } from "@voya/contracts";
import { installFakeCommands } from "@voya/features/test/backend";

const ipcMocks = installFakeCommands({
  appUpdateStatus: vi.fn(),
  updateGeoAssets: vi.fn(),
  updateSrsAssets: vi.fn(),
});
const tauriMocks = vi.hoisted(() => ({
  check: vi.fn(),
  getVersion: vi.fn(),
  relaunch: vi.fn(),
}));

vi.mock("@/ipc/tauri-plugins", () => tauriMocks);

describe("UpdatesPanel", () => {
  beforeEach(async () => {
    cleanup();
    vi.clearAllMocks();
    await changeLocale("en");
    usePreferencesStore.setState({ ruleLibraryUpdatedAt: null });
    mockDefaultIpc();
  });

  afterEach(() => cleanup());

  it("checks, confirms, installs, and restarts through the signed app updater", async () => {
    const user = userEvent.setup();
    const checkedUpdate = makeTauriUpdate();
    const installedUpdate = makeTauriUpdate();
    tauriMocks.check.mockResolvedValueOnce(checkedUpdate).mockResolvedValueOnce(installedUpdate);

    renderPanel();

    expect(await screen.findByText("Ready to check for a new version.")).toBeInTheDocument();
    expect(screen.getByText("Current version 1.0.0")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await screen.findByText("App 2.1.0 is available")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Install now" }));
    const confirm = await screen.findByRole("alertdialog");
    expect(confirm).toHaveTextContent("Install VoyaVPN 2.1.0?");
    expect(installedUpdate.downloadAndInstall).not.toHaveBeenCalled();
    await user.click(within(confirm).getByRole("button", { name: "Download and install" }));
    await waitFor(() => expect(installedUpdate.downloadAndInstall).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("App update installed: 2.1.0")).toBeInTheDocument();
    expect(screen.getByText("Restart VoyaVPN to finish applying the update.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Restart app" }));
    await waitFor(() => expect(tauriMocks.relaunch).toHaveBeenCalledTimes(1));
  });

  it("shows download progress while an update installs", async () => {
    const user = userEvent.setup();
    let finish!: () => void;
    const installed = makeTauriUpdate({
      downloadAndInstall: vi.fn((onEvent: (event: unknown) => void) => {
        onEvent({ data: { contentLength: 200 }, event: "Started" });
        onEvent({ data: { chunkLength: 100 }, event: "Progress" });
        return new Promise<void>((resolve) => {
          finish = resolve;
        });
      }),
    });
    tauriMocks.check.mockResolvedValueOnce(makeTauriUpdate()).mockResolvedValueOnce(installed);
    renderPanel();

    await user.click(await screen.findByRole("button", { name: "Check for updates" }));
    await user.click(await screen.findByRole("button", { name: "Install now" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Download and install" }),
    );

    expect(await screen.findByText("Downloading 50%")).toBeInTheDocument();
    finish();
    expect(await screen.findByText("App update installed: 2.1.0")).toBeInTheDocument();
    expect(screen.queryByText("Downloading 50%")).not.toBeInTheDocument();
  });

  it("waits for submitted settings before installing an update", async () => {
    const user = userEvent.setup();
    const client = createTestQueryClient();
    let finish!: () => void;
    const write = new Promise<void>((resolve) => { finish = resolve; });
    saveQueue(client).enqueue({}, () => write);
    const installed = makeTauriUpdate();
    tauriMocks.check.mockResolvedValueOnce(makeTauriUpdate()).mockResolvedValueOnce(installed);
    renderWithQuery(<UpdatesPanel />, { queryClient: client });
    await user.click(await screen.findByRole("button", { name: "Check for updates" }));
    await user.click(await screen.findByRole("button", { name: "Install now" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Download and install" }),
    );
    expect(installed.downloadAndInstall).not.toHaveBeenCalled();
    finish();
    await waitFor(() => expect(installed.downloadAndInstall).toHaveBeenCalledTimes(1));
  });

  it("updates the whole rule library at once and remembers when", async () => {
    const user = userEvent.setup();
    ipcMocks.updateGeoAssets.mockResolvedValue([
      makeResource("geoip.db", 123, true),
      makeResource("geosite.db", 456, false),
    ]);
    ipcMocks.updateSrsAssets.mockResolvedValue([makeResource("geosite-cn.srs", 789, false)]);

    renderPanel();

    const library = await screen.findByRole("region", { name: "IP, domain and rule set data" });
    expect(library).toHaveTextContent("Not updated from this device yet");
    await user.click(within(library).getByRole("button", { name: "Update now" }));

    await waitFor(() => expect(ipcMocks.updateSrsAssets).toHaveBeenCalledTimes(1));
    expect(ipcMocks.updateGeoAssets).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(library).toHaveTextContent("geoip.db, geosite.db, geosite-cn.srs"));
    expect(library).toHaveTextContent("Updated 3 files");
    expect(library).toHaveTextContent("Last updated");
    expect(usePreferencesStore.getState().ruleLibraryUpdatedAt).toEqual(expect.any(Number));
  });

  it("redacts a rule library failure and offers a retry", async () => {
    const user = userEvent.setup();
    ipcMocks.updateSrsAssets.mockRejectedValue(
      new Error("failed at https://rules.example/secret proxyUrl=http://127.0.0.1:10808"),
    );

    renderPanel();

    const library = await screen.findByRole("region", { name: "IP, domain and rule set data" });
    await user.click(within(library).getByRole("button", { name: "Update now" }));

    await waitFor(() => expect(library).toHaveTextContent("[redacted URL]"));
    expect(library).toHaveTextContent("proxyUrl=[redacted]");
    expect(library).not.toHaveTextContent("rules.example");
    expect(within(library).getByRole("button", { name: "Retry" })).toBeInTheDocument();
    expect(usePreferencesStore.getState().ruleLibraryUpdatedAt).toBeNull();
  });

  it("does not render update preferences or manual download fallback", async () => {
    renderPanel();

    expect(await screen.findByText("Ready to check for a new version.")).toBeInTheDocument();
    expect(screen.queryByText("Pre-release")).not.toBeInTheDocument();
    expect(screen.queryByText("Manual downloads")).not.toBeInTheDocument();
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });
});

function renderPanel() {
  return renderWithQuery(<UpdatesPanel />);
}

function mockDefaultIpc() {
  ipcMocks.appUpdateStatus.mockResolvedValue({
    currentVersion: "1.0.0",
    message: null,
    state: "ready",
  } satisfies AppUpdaterStatus);
  ipcMocks.updateGeoAssets.mockResolvedValue([]);
  ipcMocks.updateSrsAssets.mockResolvedValue([]);
  tauriMocks.check.mockResolvedValue(null);
  tauriMocks.getVersion.mockResolvedValue("1.0.0");
  tauriMocks.relaunch.mockResolvedValue(undefined);
}

function makeTauriUpdate(overrides: Record<string, unknown> = {}) {
  return {
    body: null,
    close: vi.fn().mockResolvedValue(undefined),
    currentVersion: "1.0.0",
    date: null,
    downloadAndInstall: vi.fn().mockResolvedValue(undefined),
    version: "2.1.0",
    ...overrides,
  };
}

function makeResource(name: string, bytes: number, usedProxy: boolean): ResourceUpdateFile {
  return { bytes, name, usedProxy };
}
