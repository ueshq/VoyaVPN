import { check as checkForTauriUpdate, getVersion, type Update as TauriUpdate } from "@/ipc/tauri-plugins";

/** Progress granularity while the download size is unknown. */
const PROGRESS_STEP_BYTES = 256 * 1024;

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

export async function checkAppUpdate(): Promise<AppUpdateCheckResult> {
  let update: TauriUpdate | null = null;

  try {
    const currentVersion = await getVersion();
    update = await checkForTauriUpdate();

    return {
      currentVersion,
      update: update ? appUpdateInfo(update, currentVersion) : null,
    };
  } finally {
    await closeUpdate(update);
  }
}

export async function installCheckedAppUpdate(
  onProgress?: (progress: AppUpdateProgress) => void,
): Promise<AppUpdateInstallResult> {
  let update: TauriUpdate | null = null;

  try {
    const currentVersion = await getVersion();
    update = await checkForTauriUpdate();

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
    let reported = 0;
    await update.downloadAndInstall((event: DownloadEvent) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? null;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        // One event per network chunk is thousands of them; the panel shows a
        // whole percentage, so only a step it can show is worth a render.
        const step = total ? total / 100 : PROGRESS_STEP_BYTES;
        if (downloaded - reported < step && downloaded !== total) return;
        reported = downloaded;
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
