import { appUpdateStatus } from "@/ipc/commands";
import type { AppUpdaterStatus } from "@/ipc/bindings";
import { check as checkForTauriUpdate, getVersion, type Update as TauriUpdate } from "@/ipc/updater";

type AppUpdateInfo = {
  currentVersion: string;
  version: string;
  date: string | null;
  body: string | null;
};

export type AppUpdateCheckResult = {
  currentVersion: string;
  update: AppUpdateInfo | null;
};

export type AppUpdateInstallResult = {
  state: "installed" | "noUpdate";
  currentVersion: string;
  installedVersion: string | null;
  restartRequired: boolean;
};

/** How far an install's download has got; `total` is null when the server sends no size. */
export type AppUpdateProgress = {
  downloaded: number;
  finished: boolean;
  total: number | null;
};

type DownloadEvent = Parameters<
  NonNullable<Parameters<TauriUpdate["downloadAndInstall"]>[0]>
>[0];

export type AppUpdateFlowDeps = {
  appUpdateStatus: typeof appUpdateStatus;
  checkForAppUpdate: typeof checkForTauriUpdate;
  getCurrentVersion: typeof getVersion;
};

const defaultAppUpdateFlowDeps: AppUpdateFlowDeps = {
  appUpdateStatus,
  checkForAppUpdate: checkForTauriUpdate,
  getCurrentVersion: getVersion,
};

export async function loadAppUpdaterStatus(
  deps: AppUpdateFlowDeps = defaultAppUpdateFlowDeps,
): Promise<AppUpdaterStatus> {
  return deps.appUpdateStatus();
}

export async function checkAppUpdate(
  deps: AppUpdateFlowDeps = defaultAppUpdateFlowDeps,
): Promise<AppUpdateCheckResult> {
  return checkForAppUpdate(deps);
}

export async function installCheckedAppUpdate(
  deps: AppUpdateFlowDeps = defaultAppUpdateFlowDeps,
  onProgress?: (progress: AppUpdateProgress) => void,
): Promise<AppUpdateInstallResult> {
  let update: TauriUpdate | null = null;

  try {
    const currentVersion = await deps.getCurrentVersion();
    update = await deps.checkForAppUpdate();

    if (!update) {
      return {
        currentVersion,
        installedVersion: null,
        restartRequired: false,
        state: "noUpdate",
      };
    }

    const installedVersion = update.version;
    let downloaded = 0;
    let total: number | null = null;
    await update.downloadAndInstall((event: DownloadEvent) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? null;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
      }
      onProgress?.({ downloaded, finished: event.event === "Finished", total });
    });

    return {
      currentVersion,
      installedVersion,
      restartRequired: true,
      state: "installed",
    };
  } finally {
    await closeUpdate(update);
  }
}

async function checkForAppUpdate(
  deps: AppUpdateFlowDeps,
): Promise<AppUpdateCheckResult> {
  let update: TauriUpdate | null = null;

  try {
    const currentVersion = await deps.getCurrentVersion();
    update = await deps.checkForAppUpdate();

    return {
      currentVersion,
      update: update ? appUpdateInfo(update, currentVersion) : null,
    };
  } finally {
    await closeUpdate(update);
  }
}

function appUpdateInfo(update: TauriUpdate, fallbackCurrentVersion: string): AppUpdateInfo {
  return {
    body: update.body ?? null,
    currentVersion: update.currentVersion || fallbackCurrentVersion,
    date: update.date ?? null,
    version: update.version,
  };
}

async function closeUpdate(update: TauriUpdate | null) {
  try {
    await update?.close();
  } catch {
    // Resource cleanup is best-effort because some install paths close in Rust.
  }
}
