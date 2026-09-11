import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { settingsSaveQueue } from "@/features/settings/settings-save-queue";
import { UpdatesPanel } from "@/features/updates/updates-panel";
import { changeLocale } from "@voya/i18n";
import type { AppUpdaterStatus, ResourceUpdateFile } from "@/ipc/bindings";

const ipcMocks = vi.hoisted(() => ({
  appUpdateStatus: vi.fn(),
  updateGeoAssets: vi.fn(),
  updateSrsAssets: vi.fn(),
}));
const tauriMocks = vi.hoisted(() => ({
  check: vi.fn(),
  getVersion: vi.fn(),
  relaunch: vi.fn(),
}));

vi.mock("@/ipc/commands", () => ipcMocks);
vi.mock("@/ipc/process", () => ({ relaunch: tauriMocks.relaunch }));
vi.mock("@/ipc/updater", () => ({
  check: tauriMocks.check,
  getVersion: tauriMocks.getVersion,
}));

describe("UpdatesPanel", () => {
  beforeEach(async () => {
    cleanup();
    vi.clearAllMocks();
    await changeLocale("en");
    mockDefaultIpc();
  });

  afterEach(() => cleanup());

  it("checks, installs, and restarts through the signed app updater", async () => {
    const user = userEvent.setup();
    const checkedUpdate = makeTauriUpdate();
    const installedUpdate = makeTauriUpdate();
    tauriMocks.check.mockResolvedValueOnce(checkedUpdate).mockResolvedValueOnce(installedUpdate);

    render(<QueryClientProvider client={new QueryClient()}><UpdatesPanel /></QueryClientProvider>);

    expect(await screen.findByText("Automatic app updater is ready.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Check app" }));
    expect(await screen.findByText("App 2.1.0 is available")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Install app" }));
    await waitFor(() => expect(installedUpdate.downloadAndInstall).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("App update installed: 2.1.0")).toBeInTheDocument();
    expect(screen.getByText("Restart VoyaVPN to finish applying the update.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Restart app" }));
    await waitFor(() => expect(tauriMocks.relaunch).toHaveBeenCalledTimes(1));
  });

  it("waits for submitted settings before installing an update", async () => {
    const user = userEvent.setup();
    const client = new QueryClient();
    let finish!: () => void;
    const write = new Promise<void>((resolve) => { finish = resolve; });
    settingsSaveQueue(client).enqueue({}, () => write);
    const installed = makeTauriUpdate();
    tauriMocks.check.mockResolvedValueOnce(makeTauriUpdate()).mockResolvedValueOnce(installed);
    render(<QueryClientProvider client={client}><UpdatesPanel /></QueryClientProvider>);
    await user.click(await screen.findByRole("button", { name: "Check app" }));
    await user.click(await screen.findByRole("button", { name: "Install app" }));
    expect(installed.downloadAndInstall).not.toHaveBeenCalled();
    finish();
    await waitFor(() => expect(installed.downloadAndInstall).toHaveBeenCalledTimes(1));
  });

  it("updates Geo independently and displays the returned files", async () => {
    const user = userEvent.setup();
    ipcMocks.updateGeoAssets.mockResolvedValue([
      makeResource("geoip.db", 123, true),
      makeResource("geosite.db", 456, false),
    ]);

    render(<QueryClientProvider client={new QueryClient()}><UpdatesPanel /></QueryClientProvider>);

    const geo = await screen.findByRole("region", { name: "Geo assets" });
    await user.click(within(geo).getByRole("button", { name: "Update now" }));

    await waitFor(() => expect(ipcMocks.updateGeoAssets).toHaveBeenCalledTimes(1));
    expect(ipcMocks.updateSrsAssets).not.toHaveBeenCalled();
    expect(geo).toHaveTextContent("geoip.db, geosite.db");
    expect(geo).toHaveTextContent("Updated 2 files");
  });

  it("updates SRS independently and redacts failure details", async () => {
    const user = userEvent.setup();
    ipcMocks.updateSrsAssets.mockRejectedValue(
      new Error("failed at https://rules.example/secret proxyUrl=http://127.0.0.1:10808"),
    );

    render(<QueryClientProvider client={new QueryClient()}><UpdatesPanel /></QueryClientProvider>);

    const srs = await screen.findByRole("region", { name: "SRS assets" });
    await user.click(within(srs).getByRole("button", { name: "Update now" }));

    await waitFor(() => expect(ipcMocks.updateSrsAssets).toHaveBeenCalledTimes(1));
    expect(srs).toHaveTextContent("[redacted URL]");
    expect(srs).toHaveTextContent("proxyUrl=[redacted]");
    expect(srs).not.toHaveTextContent("rules.example");
  });

  it("does not render update preferences or manual download fallback", async () => {
    render(<QueryClientProvider client={new QueryClient()}><UpdatesPanel /></QueryClientProvider>);

    expect(await screen.findByText("Automatic app updater is ready.")).toBeInTheDocument();
    expect(screen.queryByText("Pre-release")).not.toBeInTheDocument();
    expect(screen.queryByText("Manual downloads")).not.toBeInTheDocument();
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });
});

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
