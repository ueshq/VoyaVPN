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

  it("checks, downloads, and installs an app update", async () => {
    const user = userEvent.setup();

    renderDialog();

    expect(await screen.findByText("Manual 2.1.0 available")).toBeInTheDocument();
    expect(screen.getByText("App")).toBeInTheDocument();
    expect(screen.getByText("Geo")).toBeInTheDocument();
    expect(screen.getByText("SRS")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Check" }));

    await waitFor(() =>
      expect(ipcMocks.checkUpdates).toHaveBeenCalledWith(false, ["app", "geo", "srs"], true, null),
    );
    expect(await screen.findByText("Available 2.1.0")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Download" }));

    await waitFor(() =>
      expect(ipcMocks.downloadUpdates).toHaveBeenCalledWith(false, ["app", "geo", "srs"], true, null),
    );
    await waitFor(() => expect(screen.getAllByText("Downloaded")).toHaveLength(2));

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

    const srs = await screen.findByRole("checkbox", { name: "Selected sing-box rulesets" });
    const geo = screen.getByRole("checkbox", { name: "Selected Geo files" });

    await user.click(srs);

    expect(srs).not.toBeChecked();
    await waitFor(() => expect(ipcMocks.saveUpdatePreferences).toHaveBeenCalledTimes(1));
    expect(saves[0]?.preRelease).toBe(false);
    expect(saves[0]?.selectedIds).toEqual(["app", "geo"]);

    await user.click(geo);

    expect(geo).not.toBeChecked();
    expect(ipcMocks.saveUpdatePreferences).toHaveBeenCalledTimes(1);

    await act(async () => {
      saves[0]?.deferred.resolve(makeStatus(selectTargets(["app", "geo"])));
      await Promise.resolve();
    });

    await waitFor(() => expect(ipcMocks.saveUpdatePreferences).toHaveBeenCalledTimes(2));
    expect(saves[1]?.preRelease).toBe(false);
    expect(saves[1]?.selectedIds).toEqual(["app"]);
    expect(srs).not.toBeChecked();
    expect(geo).not.toBeChecked();

    await act(async () => {
      saves[1]?.deferred.resolve(makeStatus(selectTargets(["app"])));
      await Promise.resolve();
    });

    await waitFor(() => expect(srs).not.toBeChecked());
    expect(geo).not.toBeChecked();
  });

  it("shows the no selected target state and disables check actions", async () => {
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
}

function makeStatus(targets: UpdateTarget[], preRelease = false): UpdateStatus {
  return { preRelease, targets };
}

function makeCheckResults(): UpdateCheckResult[] {
  return [
    result("app", "updateAvailable", "2.1.0 is available", "1.0.0", "2.1.0"),
    result("geo", "updateAvailable", "geo files can be refreshed", null, null),
    result("srs", "updateAvailable", "SRS rulesets can be refreshed", null, null),
  ];
}

function makeDownloadResults(): UpdateCheckResult[] {
  return [
    result("app", "updateAvailable", "2.1.0 is available", "1.0.0", "2.1.0"),
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
