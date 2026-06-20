import {
  appUpdateStatus,
  checkAppUpdate,
  installAppUpdate,
  manualAppUpdateLinks,
} from "@/ipc";
import type {
  AppUpdateCheckResult,
  AppUpdateInstallResult,
  AppUpdaterStatus,
  ManualAppUpdateLinks,
} from "@/ipc/bindings";

export type AppUpdateFlowDeps = {
  appUpdateStatus: typeof appUpdateStatus;
  checkAppUpdate: typeof checkAppUpdate;
  installAppUpdate: typeof installAppUpdate;
  manualAppUpdateLinks: typeof manualAppUpdateLinks;
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
  checkAppUpdate,
  installAppUpdate,
  manualAppUpdateLinks,
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
    deps.checkAppUpdate(),
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
  return deps.installAppUpdate();
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
  return error instanceof Error ? error.message : String(error);
}
