import { useQueryClient } from "@tanstack/react-query";
import { settingsSaveQueue } from "@/features/settings/settings-save-queue";
import { useEffect, useRef, useState } from "react";

import {
  checkAppUpdate,
  installCheckedAppUpdate,
  type AppUpdateCheckResult,
  type AppUpdateInstallResult,
  type AppUpdateProgress,
} from "@/features/updates/app-update-flow";
import { appUpdateStatus, updateGeoAssets, updateSrsAssets } from "@/ipc/commands";
import type { AppUpdaterStatus, ResourceUpdateFile } from "@/ipc/bindings";
import { relaunch } from "@/ipc/tauri-plugins";
import { usePreferencesStore } from "@/stores/preferences-store";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { useMountedRef } from "@voya/utils/use-mounted-ref";

type UpdateWorkingState =
  | "app-check"
  | "app-install"
  | "app-restart"
  | "rule-library";

export function useCheckUpdateDialog() {
  const { t } = useI18n();
  const queue = settingsSaveQueue(useQueryClient());
  const [appUpdaterStatus, setAppUpdaterStatus] = useState<AppUpdaterStatus | null>(null);
  const [appUpdaterCheck, setAppUpdaterCheck] = useState<AppUpdateCheckResult | null>(null);
  const [appUpdaterError, setAppUpdaterError] = useState<string | null>(null);
  const [appInstallResult, setAppInstallResult] = useState<AppUpdateInstallResult | null>(null);
  const [installProgress, setInstallProgress] = useState<AppUpdateProgress | null>(null);
  const [ruleLibraryFiles, setRuleLibraryFiles] = useState<ResourceUpdateFile[] | null>(null);
  const [ruleLibraryError, setRuleLibraryError] = useState<string | null>(null);
  const ruleLibraryUpdatedAt = usePreferencesStore((state) => state.ruleLibraryUpdatedAt);
  const [working, setWorking] = useState<UpdateWorkingState | null>(null);
  const statusGenerationRef = useRef(0);
  const mountedRef = useMountedRef();

  useEffect(() => {
    const generation = ++statusGenerationRef.current;
    const isCurrent = () => mountedRef.current && generation === statusGenerationRef.current;

    void appUpdateStatus()
      .then((status) => {
        if (isCurrent()) {
          setAppUpdaterStatus(status);
        }
      })
      .catch((error: unknown) => {
        if (isCurrent()) {
          setAppUpdaterError(getErrorMessage(error));
        }
      });

    return () => {
      statusGenerationRef.current += 1;
    };
  }, [mountedRef]);

  async function runAppUpdaterCheck() {
    setWorking("app-check");
    setAppUpdaterError(null);
    setAppUpdaterCheck(null);
    setAppInstallResult(null);
    try {
      setAppUpdaterCheck(await checkAppUpdate());
    } catch (error) {
      setAppUpdaterError(getErrorMessage(error));
    } finally {
      setWorking(null);
    }
  }

  async function installAppUpdate() {
    setWorking("app-install");
    setAppUpdaterError(null);
    setAppInstallResult(null);
    setInstallProgress(null);
    try {
      await queue.settled();
      setAppInstallResult(
        await installCheckedAppUpdate((progress) => {
          if (mountedRef.current) setInstallProgress(progress);
        }),
      );
    } catch (error) {
      setAppUpdaterError(getErrorMessage(error));
    } finally {
      setInstallProgress(null);
      setWorking(null);
    }
  }

  async function restartApp() {
    setWorking("app-restart");
    setAppUpdaterError(null);
    try {
      await queue.settled();
      await relaunch();
    } catch (error) {
      setAppUpdaterError(getErrorMessage(error));
    } finally {
      setWorking(null);
    }
  }

  /**
   * The whole rule library in one go: IP and domain data, then the rule sets.
   * A failure part way keeps the files that did arrive on show.
   */
  async function updateRuleLibrary() {
    setWorking("rule-library");
    setRuleLibraryError(null);
    setRuleLibraryFiles(null);
    const files: ResourceUpdateFile[] = [];
    try {
      await queue.settled();
      files.push(...(await updateGeoAssets()));
      files.push(...(await updateSrsAssets()));
      usePreferencesStore.getState().setRuleLibraryUpdatedAt(Date.now());
    } catch (error) {
      setRuleLibraryError(getErrorMessage(error));
    } finally {
      setRuleLibraryFiles(files);
      setWorking(null);
    }
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
