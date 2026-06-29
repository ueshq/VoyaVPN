import { getVersion } from "@tauri-apps/api/app";
import { check as checkForTauriUpdate, type Update as TauriUpdate } from "@tauri-apps/plugin-updater";

import {
  appUpdateStatus,
  manualAppUpdateLinks,
  recordAppUpdateDiagnostic,
} from "@/ipc";
import type { AppUpdaterStatus, ManualAppUpdateLinks } from "@/ipc/bindings";
import { getErrorMessage } from "@/lib/utils";

export type AppUpdateInfo = {
  currentVersion: string;
  version: string;
  date: string | null;
  body: string | null;
  downloadUrl: string;
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
  manualAppUpdateLinks: typeof manualAppUpdateLinks;
  recordAppUpdateDiagnostic: typeof recordAppUpdateDiagnostic;
};

export type AppUpdatePaths = {
  updaterStatus: AppUpdaterStatus | null;
  updaterCheck: AppUpdateCheckResult | null;
  manualLinks: ManualAppUpdateLinks | null;
  updaterError: string | null;
  manualError: string | null;
};

export const defaultAppUpdateFlowDeps: AppUpdateFlowDeps = {
  appUpdateStatus,
  checkForAppUpdate: checkForTauriUpdate,
  getCurrentVersion: getVersion,
  manualAppUpdateLinks,
  recordAppUpdateDiagnostic,
};

const manualDownloadAllowedHosts = ["cdn.voyavpn.dev", "cdn.voyavpn.test"] as const;

export async function loadAppUpdatePaths(
  preRelease: boolean,
  preferProxy = true,
  proxyUrl: string | null = null,
  deps: AppUpdateFlowDeps = defaultAppUpdateFlowDeps,
): Promise<AppUpdatePaths> {
  const [updaterStatus, manualLinks] = await Promise.allSettled([
    deps.appUpdateStatus(),
    loadSafeManualLinks(preRelease, preferProxy, proxyUrl, deps),
  ]);

  return {
    updaterStatus: settledValue(updaterStatus),
    updaterCheck: null,
    manualLinks: settledValue(manualLinks),
    updaterError: settledError(updaterStatus),
    manualError: settledError(manualLinks),
  };
}

export async function checkAppUpdatePaths(
  preRelease: boolean,
  preferProxy = true,
  proxyUrl: string | null = null,
  deps: AppUpdateFlowDeps = defaultAppUpdateFlowDeps,
): Promise<AppUpdatePaths> {
  const [updaterCheck, manualLinks] = await Promise.allSettled([
    checkAndRecordAppUpdate(deps),
    loadSafeManualLinks(preRelease, preferProxy, proxyUrl, deps),
  ]);

  return {
    updaterStatus: null,
    updaterCheck: settledValue(updaterCheck),
    manualLinks: settledValue(manualLinks),
    updaterError: settledError(updaterCheck),
    manualError: settledError(manualLinks),
  };
}

export async function installCheckedAppUpdate(
  deps: AppUpdateFlowDeps = defaultAppUpdateFlowDeps,
): Promise<AppUpdateInstallResult> {
  let update: TauriUpdate | null = null;

  try {
    const currentVersion = await deps.getCurrentVersion();
    update = await deps.checkForAppUpdate();

    if (!update) {
      await recordDiagnostic(deps, "install", "skipped", null);

      return {
        currentVersion,
        installedVersion: null,
        restartRequired: false,
        state: "noUpdate",
      };
    }

    const installedVersion = update.version;
    await update.downloadAndInstall();
    await recordDiagnostic(deps, "install", "success", null);

    return {
      currentVersion,
      installedVersion,
      restartRequired: true,
      state: "installed",
    };
  } catch (error) {
    await recordDiagnostic(deps, "install", "failure", errorMessage(error));
    throw error;
  } finally {
    await closeUpdate(update);
  }
}

async function checkAndRecordAppUpdate(
  deps: AppUpdateFlowDeps,
): Promise<AppUpdateCheckResult> {
  let update: TauriUpdate | null = null;

  try {
    const currentVersion = await deps.getCurrentVersion();
    update = await deps.checkForAppUpdate();
    await recordDiagnostic(deps, "check", "success", null);

    return {
      currentVersion,
      update: update ? appUpdateInfo(update, currentVersion) : null,
    };
  } catch (error) {
    await recordDiagnostic(deps, "check", "failure", errorMessage(error));
    throw error;
  } finally {
    await closeUpdate(update);
  }
}

function appUpdateInfo(update: TauriUpdate, fallbackCurrentVersion: string): AppUpdateInfo {
  return {
    body: update.body ?? null,
    currentVersion: update.currentVersion || fallbackCurrentVersion,
    date: update.date ?? null,
    downloadUrl: rawString(update.rawJson, "downloadUrl") ?? rawString(update.rawJson, "url") ?? "",
    version: update.version,
  };
}

async function recordDiagnostic(
  deps: AppUpdateFlowDeps,
  action: "check" | "install",
  result: "success" | "failure" | "skipped",
  message: string | null,
) {
  try {
    await deps.recordAppUpdateDiagnostic(action, result, message);
  } catch {
    // Diagnostics must never block the updater path.
  }
}

async function closeUpdate(update: TauriUpdate | null) {
  try {
    await update?.close();
  } catch {
    // Resource cleanup is best-effort because some install paths close in Rust.
  }
}

async function loadSafeManualLinks(
  preRelease: boolean,
  preferProxy: boolean,
  proxyUrl: string | null,
  deps: AppUpdateFlowDeps,
) {
  return assertManualLinksSafe(await deps.manualAppUpdateLinks(preRelease, preferProxy, proxyUrl));
}

export function assertManualLinksSafe(links: ManualAppUpdateLinks): ManualAppUpdateLinks {
  const forbidden = links.downloads.find((download) => isForbiddenManualUrl(download.url));
  if (forbidden) {
    throw new Error("Manual CDN release index returned a forbidden download URL.");
  }

  return links;
}

function isForbiddenManualUrl(url: string) {
  const value = url.trim();

  if (value.length === 0) {
    return true;
  }

  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    return true;
  }

  if (parsed.protocol !== "https:") {
    return true;
  }

  const hostname = parsed.hostname.toLowerCase();
  return !manualDownloadAllowedHosts.some(
    (allowedHost) => hostname === allowedHost || hostname.endsWith(`.${allowedHost}`),
  );
}

function settledValue<T>(result: PromiseSettledResult<T>): T | null {
  return result.status === "fulfilled" ? result.value : null;
}

function settledError<T>(result: PromiseSettledResult<T>): string | null {
  return result.status === "rejected" ? errorMessage(result.reason) : null;
}

function errorMessage(error: unknown) {
  return getErrorMessage(error);
}

function rawString(rawJson: Record<string, unknown>, key: string) {
  const value = rawJson[key];

  return typeof value === "string" ? value : null;
}
