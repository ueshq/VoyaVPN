import { useQueryClient } from "@tanstack/react-query";
import { settingsSaveQueue } from "@/features/settings/settings-save-queue";
import { useEffect, useRef, useState } from "react";

import {
  checkAppUpdate,
  installCheckedAppUpdate,
  loadAppUpdaterStatus,
  type AppUpdateCheckResult,
  type AppUpdateInstallResult,
} from "@/features/updates/app-update-flow";
import { updateGeoAssets, updateSrsAssets } from "@/ipc";
import type { AppUpdaterStatus, ResourceUpdateFile } from "@/ipc/bindings";
import { relaunch } from "@/ipc/process";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { useMountedRef } from "@voya/utils/use-mounted-ref";

export type UpdateWorkingState =
  | "app-check"
  | "app-install"
  | "app-restart"
  | "geo"
  | "srs";

export function useCheckUpdateDialog() {
  const { t } = useI18n();
  const queue = settingsSaveQueue(useQueryClient());
  const [appUpdaterStatus, setAppUpdaterStatus] = useState<AppUpdaterStatus | null>(null);
  const [appUpdaterCheck, setAppUpdaterCheck] = useState<AppUpdateCheckResult | null>(null);
  const [appUpdaterError, setAppUpdaterError] = useState<string | null>(null);
  const [appInstallResult, setAppInstallResult] = useState<AppUpdateInstallResult | null>(null);
  const [resourceResults, setResourceResults] = useState<Record<"geo" | "srs", ResourceUpdateFile[] | null>>({
    geo: null,
    srs: null,
  });
  const [resourceErrors, setResourceErrors] = useState<Record<"geo" | "srs", string | null>>({
    geo: null,
    srs: null,
  });
  const [working, setWorking] = useState<UpdateWorkingState | null>(null);
  const statusGenerationRef = useRef(0);
  const mountedRef = useMountedRef();

  useEffect(() => {
    const generation = ++statusGenerationRef.current;
    const isCurrent = () => mountedRef.current && generation === statusGenerationRef.current;

    void loadAppUpdaterStatus()
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
    try {
      await queue.settled();
      setAppInstallResult(await installCheckedAppUpdate());
    } catch (error) {
      setAppUpdaterError(getErrorMessage(error));
    } finally {
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

  async function updateResource(kind: "geo" | "srs") {
    setWorking(kind);
    setResourceErrors((current) => ({ ...current, [kind]: null }));
    setResourceResults((current) => ({ ...current, [kind]: null }));
    try {
      await queue.settled();
      const result = kind === "geo" ? await updateGeoAssets() : await updateSrsAssets();
      setResourceResults((current) => ({ ...current, [kind]: result }));
    } catch (error) {
      setResourceErrors((current) => ({ ...current, [kind]: getErrorMessage(error) }));
    } finally {
      setWorking(null);
    }
  }

  return {
    appInstallResult,
    appUpdaterCheck,
    appUpdaterError,
    appUpdaterStatus,
    installAppUpdate,
    resourceErrors,
    resourceResults,
    restartApp,
    runAppUpdaterCheck,
    t,
    updateResource,
    working,
  };
}

export type CheckUpdateDialogController = ReturnType<typeof useCheckUpdateDialog>;
