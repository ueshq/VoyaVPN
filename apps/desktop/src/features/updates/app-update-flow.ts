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
    await update.downloadAndInstall();

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
