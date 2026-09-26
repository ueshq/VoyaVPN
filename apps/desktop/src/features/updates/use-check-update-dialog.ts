import { redactOperationalError } from "@voya/utils/operational-redaction";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { saveQueue } from "@voya/features/forms/save-queue";
import { useRuleLibraryUpdate } from "@voya/features/updates/use-rule-library-update";
import { useState } from "react";

import {
  checkAppUpdate,
  installCheckedAppUpdate,
  type AppUpdateCheckResult,
  type AppUpdateInstallResult,
  type AppUpdateProgress,
} from "@/features/updates/app-update-flow";
import { voyaCommands } from "@voya/client/transport";
import type { AppUpdaterStatus } from "@voya/contracts";
import { queryKeys } from "@voya/client/query-keys";
import { relaunch } from "@/ipc/tauri-plugins";
import { useI18n } from "@voya/i18n/use-i18n";
import { useMountedRef } from "@voya/utils/use-mounted-ref";

type UpdateWorkingState = "app-check" | "app-install" | "app-restart";

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
  // The rule library is the half a phone has too, so it lives in the shared
  // package; the self-update around it is desktop-only.
  const ruleLibrary = useRuleLibraryUpdate();
  const [working, setWorking] = useState<UpdateWorkingState | null>(null);
  const mountedRef = useMountedRef();

  // One fetch per dialog open, like the effect this replaced: no retries, no
  // background refetch, so a status failure shows exactly when it happens.
  const statusQuery = useQuery({
    queryFn: () => voyaCommands().appUpdateStatus(),
    queryKey: queryKeys.appUpdaterStatus,
    refetchOnWindowFocus: false,
    retry: false,
  });
  const appUpdaterError =
    appActionError ??
    (statusQuery.error ? redactOperationalError(statusQuery.error) : null);
  const appUpdaterStatus: AppUpdaterStatus | null = statusQuery.data ?? null;

  const appUpdaterSink: ErrorSink = {
    clear: () => setAppActionError(null),
    fail: setAppActionError,
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
      errorSink.fail(redactOperationalError(error));
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

  return {
    appInstallResult,
    appUpdaterCheck,
    appUpdaterError,
    appUpdaterStatus,
    installAppUpdate,
    installProgress,
    restartApp,
    ruleLibraryError: ruleLibrary.error,
    ruleLibraryFiles: ruleLibrary.files,
    ruleLibraryUpdatedAt: ruleLibrary.updatedAt,
    runAppUpdaterCheck,
    t,
    updateRuleLibrary: ruleLibrary.update,
    // One busy marker for the dialog, whichever half is working.
    working: ruleLibrary.updating ? ("rule-library" as const) : working,
  };
}

export type CheckUpdateDialogController = ReturnType<typeof useCheckUpdateDialog>;
