import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { Dialog } from "@/components/ui/dialog";
import { CheckUpdateDialog } from "@/features/updates/check-update-dialog";
import { changeLocale } from "@/i18n";
import type {
  AppUpdaterStatus,
  ManualAppUpdateLinks,
  UpdateCheckResult,
  UpdateStatus,
  UpdateTarget,
} from "@/ipc/bindings";

const ipcMocks = vi.hoisted(() => ({
  appUpdateStatus: vi.fn(),
  applyDownloadedCoreUpdate: vi.fn(),
  checkAppUpdate: vi.fn(),
  checkUpdates: vi.fn(),
  downloadUpdates: vi.fn(),
  installAppUpdate: vi.fn(),
  manualAppUpdateLinks: vi.fn(),
  saveUpdatePreferences: vi.fn(),
  updateStatus: vi.fn(),
}));

vi.mock("@/ipc", () => ipcMocks);

describe("CheckUpdateDialog", () => {
  beforeEach(async () => {
    cleanup();
    vi.clearAllMocks();
    await changeLocale("en");
    mockDefaultIpc();
  });

  afterEach(() => {
    cleanup();
  });

  it("checks, downloads, applies a core update, and installs an app update", async () => {
    const user = userEvent.setup();

    renderDialog();

    expect(await screen.findByText("Manual 2.1.0 available")).toBeInTheDocument();
    expect(screen.getByText("App")).toBeInTheDocument();
    expect(screen.getByText("Core")).toBeInTheDocument();
    expect(screen.getByText("Geo")).toBeInTheDocument();
    expect(screen.getByText("SRS")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Check" }));

    await waitFor(() =>
      expect(ipcMocks.checkUpdates).toHaveBeenCalledWith(false, ["app", "xray", "geo", "srs"], true, null),
    );
    expect(await screen.findByText("Available 2.0.0")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Download" }));

    await waitFor(() =>
      expect(ipcMocks.downloadUpdates).toHaveBeenCalledWith(false, ["app", "xray", "geo", "srs"], true, null),
    );
    expect(await screen.findByText("Downloaded 2.0.0")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Apply" }));

    await waitFor(() =>
      expect(ipcMocks.applyDownloadedCoreUpdate).toHaveBeenCalledWith({
        fileName: "/tmp/voyavpn/xray.zip",
        remoteVersion: "2.0.0",
        sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        targetId: "xray",
      }),
    );
    expect(await screen.findByText("Applied 2.0.0")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Install app" }));

    await waitFor(() => expect(ipcMocks.installAppUpdate).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("App update installed: 2.1.0")).toBeInTheDocument();
  });

  it("shows manual CDN links when the automatic updater status fails", async () => {
    ipcMocks.appUpdateStatus.mockRejectedValue(
      new Error("updater failed at https://updates.voyavpn.test/latest.json proxyUrl=http://127.0.0.1:8080"),
    );

    renderDialog();

    const link = await screen.findByRole("link", {
      name: "APPIMAGE VoyaVPN-linux-x64.AppImage",
    });

    expect(link).toHaveAttribute("href", "https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
    expect(screen.getByText(/updater failed at/)).toHaveTextContent("[redacted URL]");
    expect(screen.getByText(/updater failed at/)).toHaveTextContent("proxyUrl=[redacted]");
    expect(screen.queryByText(/updates\.voyavpn\.test/)).not.toBeInTheDocument();
  });

  it("keeps the downloaded row actionable and failed when core apply fails", async () => {
    const user = userEvent.setup();
    ipcMocks.applyDownloadedCoreUpdate.mockRejectedValue(
      new Error("checksum mismatch for https://cdn.voyavpn.test/stable/xray.zip proxyUrl=http://127.0.0.1:8080"),
    );

    renderDialog();

    await screen.findByText("Manual 2.1.0 available");
    await user.click(screen.getByRole("button", { name: "Download" }));
    await screen.findByText("Downloaded 2.0.0");
    await user.click(screen.getByRole("button", { name: "Apply" }));

    const failedMessage = await screen.findByText(/Failed: checksum mismatch/);
    expect(failedMessage).toHaveTextContent("[redacted URL]");
    expect(failedMessage).toHaveTextContent("proxyUrl=[redacted]");
    expect(screen.queryByText(/cdn\.voyavpn\.test\/stable\/xray\.zip/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Apply" })).toBeEnabled();
  });

  it("serializes preference saves and ignores stale preference responses", async () => {
    const user = userEvent.setup();
    const saves: Array<{
      preRelease: boolean;
      selectedIds: string[];
      deferred: Deferred<UpdateStatus>;
    }> = [];
    ipcMocks.saveUpdatePreferences.mockImplementation((preRelease: boolean, selectedIds: string[]) => {
      const deferred = createDeferred<UpdateStatus>();
      saves.push({ deferred, preRelease, selectedIds });

      return deferred.promise;
    });

    renderDialog();

    const xray = await screen.findByRole("checkbox", { name: "Selected Xray" });
    const geo = screen.getByRole("checkbox", { name: "Selected Geo files" });

    await user.click(xray);

    expect(xray).not.toBeChecked();
    await waitFor(() => expect(ipcMocks.saveUpdatePreferences).toHaveBeenCalledTimes(1));
    expect(saves[0]?.preRelease).toBe(false);
    expect(saves[0]?.selectedIds).toEqual(["app", "geo", "srs"]);

    await user.click(geo);

    expect(geo).not.toBeChecked();
    expect(ipcMocks.saveUpdatePreferences).toHaveBeenCalledTimes(1);

    await act(async () => {
      saves[0]?.deferred.resolve(makeStatus(selectTargets(["app", "geo", "srs"])));
      await Promise.resolve();
    });

    await waitFor(() => expect(ipcMocks.saveUpdatePreferences).toHaveBeenCalledTimes(2));
    expect(saves[1]?.preRelease).toBe(false);
    expect(saves[1]?.selectedIds).toEqual(["app", "srs"]);
    expect(xray).not.toBeChecked();
    expect(geo).not.toBeChecked();

    await act(async () => {
      saves[1]?.deferred.resolve(makeStatus(selectTargets(["app", "srs"])));
      await Promise.resolve();
    });

    await waitFor(() => expect(xray).not.toBeChecked());
    expect(geo).not.toBeChecked();
  });

  it("refreshes backend status after applying a core update", async () => {
    const user = userEvent.setup();
    ipcMocks.updateStatus
      .mockResolvedValueOnce(makeStatus(allTargets))
      .mockResolvedValueOnce(makeStatus(selectTargets(["app", "geo", "srs"]), true));

    renderDialog();

    await screen.findByText("Manual 2.1.0 available");
    await user.click(screen.getByRole("button", { name: "Download" }));
    await screen.findByText("Downloaded 2.0.0");
    await user.click(screen.getByRole("button", { name: "Apply" }));

    await waitFor(() => expect(ipcMocks.updateStatus).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("Applied 2.0.0")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Pre-release" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Selected Xray" })).not.toBeChecked();
  });

  it("shows the no selected target state and disables core check actions", async () => {
    ipcMocks.updateStatus.mockResolvedValue(makeStatus(allTargets.map((target) => ({ ...target, selected: false }))));

    renderDialog();

    expect(await screen.findByText("Select at least one target to check or download.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Check" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Download" })).toBeDisabled();
  });
});

function renderDialog() {
  return render(
    <Dialog open>
      <CheckUpdateDialog />
    </Dialog>,
  );
}

function mockDefaultIpc() {
  ipcMocks.updateStatus.mockResolvedValue(makeStatus(allTargets));
  ipcMocks.saveUpdatePreferences.mockImplementation(async (preRelease: boolean, selectedIds: string[]) =>
    makeStatus(
      allTargets.map((target) => ({
        ...target,
        selected: selectedIds.includes(target.id),
      })),
      preRelease,
    ),
  );
  ipcMocks.appUpdateStatus.mockResolvedValue({
    currentVersion: "1.0.0",
    message: null,
    state: "ready",
  } satisfies AppUpdaterStatus);
  ipcMocks.checkAppUpdate.mockResolvedValue({
    currentVersion: "1.0.0",
    update: {
      body: null,
      currentVersion: "1.0.0",
      date: null,
      downloadUrl: "https://cdn.voyavpn.test/stable/latest.json",
      version: "2.1.0",
    },
  });
  ipcMocks.installAppUpdate.mockResolvedValue({
    currentVersion: "1.0.0",
    installedVersion: "2.1.0",
    state: "installed",
  });
  ipcMocks.manualAppUpdateLinks.mockResolvedValue(makeManualLinks());
  ipcMocks.checkUpdates.mockResolvedValue({
    preRelease: false,
    results: makeCheckResults(),
    targets: allTargets,
  });
  ipcMocks.downloadUpdates.mockResolvedValue({
    preRelease: false,
    results: makeDownloadResults(),
    targets: allTargets,
  });
  ipcMocks.applyDownloadedCoreUpdate.mockResolvedValue({
    appliedVersion: "2.0.0",
    coreType: 2,
    rollbackPath: null,
    targetDir: "/tmp/voyavpn/bin/xray",
  });
}

function makeStatus(targets: UpdateTarget[], preRelease = false): UpdateStatus {
  return { preRelease, targets };
}

function makeCheckResults(): UpdateCheckResult[] {
  return [
    result("app", "updateAvailable", "2.1.0 is available", "1.0.0", "2.1.0"),
    result("xray", "updateAvailable", "2.0.0 is available", "1.0.0", "2.0.0"),
    result("geo", "updateAvailable", "geo files can be refreshed", null, null),
    result("srs", "updateAvailable", "SRS rulesets can be refreshed", null, null),
  ];
}

function makeDownloadResults(): UpdateCheckResult[] {
  return [
    result("app", "updateAvailable", "2.1.0 is available", "1.0.0", "2.1.0"),
    {
      ...result("xray", "downloaded", "downloaded /tmp/voyavpn/xray.zip", "1.0.0", "2.0.0"),
      bytes: 123,
      fileName: "/tmp/voyavpn/xray.zip",
      sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    },
    {
      ...result("geo", "downloaded", "downloaded /tmp/voyavpn/geo.dat", null, null),
      bytes: 456,
      fileName: "/tmp/voyavpn/geo.dat",
    },
    {
      ...result("srs", "downloaded", "downloaded /tmp/voyavpn/rules.srs", null, null),
      bytes: 789,
      fileName: "/tmp/voyavpn/rules.srs",
    },
  ];
}

function result(
  targetId: string,
  status: UpdateCheckResult["status"],
  message: string,
  currentVersion: string | null,
  remoteVersion: string | null,
): UpdateCheckResult {
  return {
    bytes: null,
    currentVersion,
    downloadUrl: null,
    fileName: null,
    message,
    remoteVersion,
    sha256: null,
    status,
    targetId,
    usedProxy: null,
  };
}

function makeManualLinks(): ManualAppUpdateLinks {
  return {
    arch: "x64",
    channel: "stable",
    currentVersion: "1.0.0",
    downloads: [
      {
        bytes: 10,
        kind: "appimage",
        name: "VoyaVPN-linux-x64.AppImage",
        sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        url: "https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage",
        version: "2.1.0",
      },
    ],
    hasUpdate: true,
    releaseIndexUrl: "https://cdn.voyavpn.test/stable/release-index.json",
    remoteVersion: "2.1.0",
    target: "linux",
  };
}

const allTargets: UpdateTarget[] = [
  {
    acquisition: "appPackage",
    coreType: null,
    id: "app",
    kind: "app",
    license: "MIT",
    name: "VoyaVPN",
    redistributeInInstaller: true,
    remarks: "application package update",
    selected: true,
    updateSupported: true,
  },
  {
    acquisition: "downloadOnFirstRun",
    coreType: 2,
    id: "xray",
    kind: "core",
    license: "MPL-2.0",
    name: "Xray",
    redistributeInInstaller: true,
    remarks: "download on first run; not bundled in installers",
    selected: true,
    updateSupported: true,
  },
  {
    acquisition: "optionalDownload",
    coreType: null,
    id: "geo",
    kind: "geo",
    license: null,
    name: "Geo files",
    redistributeInInstaller: false,
    remarks: "geosite.dat and geoip.dat",
    selected: true,
    updateSupported: true,
  },
  {
    acquisition: "optionalDownload",
    coreType: null,
    id: "srs",
    kind: "srs",
    license: null,
    name: "sing-box rulesets",
    redistributeInInstaller: false,
    remarks: "SRS assets derived from routing rules",
    selected: true,
    updateSupported: true,
  },
];

type Deferred<T> = {
  promise: Promise<T>;
  reject: (reason?: unknown) => void;
  resolve: (value: T | PromiseLike<T>) => void;
};

function createDeferred<T>(): Deferred<T> {
  let resolve: Deferred<T>["resolve"] | null = null;
  let reject: Deferred<T>["reject"] | null = null;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });

  if (!resolve || !reject) {
    throw new Error("Failed to create deferred promise.");
  }

  return { promise, reject, resolve };
}

function selectTargets(selectedIds: string[]) {
  const selected = new Set(selectedIds);

  return allTargets.map((target) => ({
    ...target,
    selected: selected.has(target.id),
  }));
}
