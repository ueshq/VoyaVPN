import { useQuery, useQueryClient } from "@tanstack/react-query";
import { saveQueue } from "@/lib/save-queue";
import { useState } from "react";

import {
  checkAppUpdate,
  installCheckedAppUpdate,
  type AppUpdateCheckResult,
  type AppUpdateInstallResult,
  type AppUpdateProgress,
} from "@/features/updates/app-update-flow";
import { appUpdateStatus, updateGeoAssets, updateSrsAssets } from "@/ipc/commands";
import type { AppUpdaterStatus, ResourceUpdateFile } from "@/ipc/bindings";
import { queryKeys } from "@voya/client/query-keys";
import { relaunch } from "@/ipc/tauri-plugins";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { useMountedRef } from "@voya/utils/use-mounted-ref";

type UpdateWorkingState =
  | "app-check"
  | "app-install"
  | "app-restart"
  | "rule-library";

/** Which error line an action's failure lands on. */
interface ErrorSink {
  clear: () => void;
  fail: (message: string) => void;
}

export function useCheckUpdateDialog() {
  const { t } = useI18n();
  const queue = saveQueue(useQueryClient());
  const [appUpdaterCheck, setAppUpdaterCheck] = useState<AppUpdateCheckResult | null>(null);
  const [appActionError, setAppActionError] = useState<string | null>(null);
  const [appInstallResult, setAppInstallResult] = useState<AppUpdateInstallResult | null>(null);
  const [installProgress, setInstallProgress] = useState<AppUpdateProgress | null>(null);
  const [ruleLibraryFiles, setRuleLibraryFiles] = useState<ResourceUpdateFile[] | null>(null);
  const [ruleLibraryError, setRuleLibraryError] = useState<string | null>(null);
  const ruleLibraryUpdatedAt = usePreferencesStore((state) => state.ruleLibraryUpdatedAt);
  const [working, setWorking] = useState<UpdateWorkingState | null>(null);
  const mountedRef = useMountedRef();

  // One fetch per dialog open, like the effect this replaced: no retries, no
  // background refetch, so a status failure shows exactly when it happens.
  const statusQuery = useQuery({
    queryFn: appUpdateStatus,
    queryKey: queryKeys.appUpdaterStatus,
    refetchOnWindowFocus: false,
    retry: false,
  });
  const appUpdaterError =
    appActionError ??
    (statusQuery.error ? getErrorMessage(statusQuery.error) : null);
  const appUpdaterStatus: AppUpdaterStatus | null = statusQuery.data ?? null;

  const appUpdaterSink: ErrorSink = {
    clear: () => setAppActionError(null),
    fail: setAppActionError,
  };
  const ruleLibrarySink: ErrorSink = {
    clear: () => setRuleLibraryError(null),
    fail: setRuleLibraryError,
  };

  /** Run one dialog action under its busy marker; `onSettled` runs on both paths. */
  async function withWorking(
    kind: UpdateWorkingState,
    errorSink: ErrorSink,
    run: () => Promise<void>,
    onSettled?: () => void,
  ) {
    setWorking(kind);
    errorSink.clear();
    try {
      await run();
    } catch (error) {
      errorSink.fail(getErrorMessage(error));
    } finally {
      onSettled?.();
      setWorking(null);
    }
  }

  function runAppUpdaterCheck() {
    return withWorking("app-check", appUpdaterSink, async () => {
      setAppUpdaterCheck(null);
      setAppInstallResult(null);
      setAppUpdaterCheck(await checkAppUpdate());
    });
  }

  function installAppUpdate() {
    return withWorking(
      "app-install",
      appUpdaterSink,
      async () => {
        setAppInstallResult(null);
        setInstallProgress(null);
        await queue.settled();
        setAppInstallResult(
          await installCheckedAppUpdate((progress) => {
            if (mountedRef.current) setInstallProgress(progress);
          }),
        );
      },
      () => setInstallProgress(null),
    );
  }

  function restartApp() {
    return withWorking("app-restart", appUpdaterSink, async () => {
      await queue.settled();
      await relaunch();
    });
  }

  /**
   * The whole rule library in one go: IP and domain data, then the rule sets.
   * A failure part way keeps the files that did arrive on show.
   */
  function updateRuleLibrary() {
    const files: ResourceUpdateFile[] = [];
    return withWorking(
      "rule-library",
      ruleLibrarySink,
      async () => {
        setRuleLibraryFiles(null);
        await queue.settled();
        files.push(...(await updateGeoAssets()));
        files.push(...(await updateSrsAssets()));
        usePreferencesStore.getState().setRuleLibraryUpdatedAt(Date.now());
      },
      () => setRuleLibraryFiles(files),
    );
  }

  return {
    appInstallResult,
    appUpdaterCheck,
    appUpdaterError,
    appUpdaterStatus,
    installAppUpdate,
    installProgress,
    restartApp,
    ruleLibraryError,
    ruleLibraryFiles,
    ruleLibraryUpdatedAt,
    runAppUpdaterCheck,
    t,
    updateRuleLibrary,
    working,
  };
}

export type CheckUpdateDialogController = ReturnType<typeof useCheckUpdateDialog>;
