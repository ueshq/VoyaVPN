import { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Download,
  ExternalLink,
  PackageCheck,
  RefreshCw,
} from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  checkAppUpdatePaths,
  installCheckedAppUpdate,
  loadAppUpdatePaths,
  type AppUpdatePaths,
} from "@/features/updates/app-update-flow";
import { useI18n } from "@/i18n/use-i18n";
import {
  applyDownloadedCoreUpdate,
  checkUpdates,
  downloadUpdates,
  saveUpdatePreferences,
  updateStatus,
} from "@/ipc";
import type {
  AppUpdateCheckResult,
  AppUpdateInstallResult,
  AppUpdaterStatus,
  CoreUpdateApplyResult,
  ManualAppUpdateDownload,
  ManualAppUpdateLinks,
  UpdateAcquisition,
  UpdateCheckResult,
  UpdateResultStatus,
  UpdateStatus,
  UpdateTarget,
  UpdateTargetKind,
} from "@/ipc/bindings";
import { cn } from "@/lib/utils";

type CoreRunMode = "check" | "download";
type RunMode = CoreRunMode | "app-check" | "app-install";
type PreferenceSnapshot = {
  preRelease: boolean;
  selectedIds: string[];
};

export function CheckUpdateDialog() {
  const { t } = useI18n();
  const [error, setError] = useState<string | null>(null);
  const [appUpdaterStatus, setAppUpdaterStatus] = useState<AppUpdaterStatus | null>(null);
  const [appUpdaterCheck, setAppUpdaterCheck] = useState<AppUpdateCheckResult | null>(null);
  const [appUpdaterError, setAppUpdaterError] = useState<string | null>(null);
  const [appInstallResult, setAppInstallResult] = useState<AppUpdateInstallResult | null>(null);
  const [appliedCoreResults, setAppliedCoreResults] = useState<Map<string, CoreUpdateApplyResult>>(
    new Map(),
  );
  const [applyingCoreTargetId, setApplyingCoreTargetId] = useState<string | null>(null);
  const [coreApplyErrors, setCoreApplyErrors] = useState<Map<string, string>>(new Map());
  const [manualLinks, setManualLinks] = useState<ManualAppUpdateLinks | null>(null);
  const [manualLinksError, setManualLinksError] = useState<string | null>(null);
  const [preRelease, setPreRelease] = useState(false);
  const [results, setResults] = useState<UpdateCheckResult[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [working, setWorking] = useState<RunMode | null>(null);
  const preferenceSnapshotRef = useRef<PreferenceSnapshot>({
    preRelease: false,
    selectedIds: [],
  });
  const preferenceSaveChainRef = useRef<Promise<void>>(Promise.resolve());
  const latestPreferenceRequestRef = useRef(0);
  const latestPreferenceSaveRef = useRef<{ key: string; promise: Promise<void> } | null>(null);
  const lastPersistedPreferenceKeyRef = useRef<string | null>(null);
  const pendingPreferenceSaveCountRef = useRef(0);

  function applyAppUpdatePaths(paths: AppUpdatePaths) {
    if (paths.updaterStatus) {
      setAppUpdaterStatus(paths.updaterStatus);
    }
    if (paths.updaterCheck) {
      setAppUpdaterCheck(paths.updaterCheck);
      setAppInstallResult(null);
    }
    setManualLinks(paths.manualLinks);
    setAppUpdaterError(paths.updaterError);
    setManualLinksError(paths.manualError);
  }

  useEffect(() => {
    let disposed = false;

    void updateStatus()
      .then(async (nextStatus) => {
        if (disposed) {
          return;
        }
        applyUpdateStatus(nextStatus);
        const appPaths = await loadAppUpdatePaths(nextStatus.preRelease, true, null);
        if (!disposed) {
          applyAppUpdatePaths(appPaths);
        }
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  const resultByTarget = useMemo(
    () => new Map(results.map((result) => [result.targetId, result])),
    [results],
  );

  function setPreferenceSnapshot(nextPreference: PreferenceSnapshot) {
    const snapshot = clonePreferenceSnapshot(nextPreference);
    preferenceSnapshotRef.current = snapshot;
    setPreRelease(snapshot.preRelease);
    setSelectedIds(new Set(snapshot.selectedIds));
  }

  function applyUpdateStatus(nextStatus: UpdateStatus) {
    const nextPreference = preferenceSnapshotFromStatus(nextStatus);
    lastPersistedPreferenceKeyRef.current = preferenceSnapshotKey(nextPreference);
    setStatus(nextStatus);
    setPreferenceSnapshot(nextPreference);
  }

  function persistPreference(nextPreference: PreferenceSnapshot) {
    const snapshot = clonePreferenceSnapshot(nextPreference);
    const key = preferenceSnapshotKey(snapshot);
    const latestSave = latestPreferenceSaveRef.current;

    if (pendingPreferenceSaveCountRef.current === 0 && key === lastPersistedPreferenceKeyRef.current) {
      return Promise.resolve();
    }

    if (latestSave && latestSave.key === key) {
      return latestSave.promise;
    }

    const requestId = latestPreferenceRequestRef.current + 1;
    latestPreferenceRequestRef.current = requestId;
    pendingPreferenceSaveCountRef.current += 1;

    const request = preferenceSaveChainRef.current
      .catch(() => undefined)
      .then(async () => {
        const nextStatus = await saveUpdatePreferences(snapshot.preRelease, snapshot.selectedIds);
        const savedPreference = preferenceSnapshotFromStatus(nextStatus);
        lastPersistedPreferenceKeyRef.current = preferenceSnapshotKey(savedPreference);

        if (requestId === latestPreferenceRequestRef.current) {
          setStatus(nextStatus);
          setPreferenceSnapshot(savedPreference);
        }
      });

    preferenceSaveChainRef.current = request.catch(() => undefined);
    latestPreferenceSaveRef.current = { key, promise: request };
    void request.then(
      () => clearTrackedPreferenceSave(request),
      () => clearTrackedPreferenceSave(request),
    );

    return request;
  }

  function clearTrackedPreferenceSave(request: Promise<void>) {
    pendingPreferenceSaveCountRef.current = Math.max(0, pendingPreferenceSaveCountRef.current - 1);
    if (latestPreferenceSaveRef.current?.promise === request) {
      latestPreferenceSaveRef.current = null;
    }
  }

  async function waitForPendingPreferenceSaves() {
    await preferenceSaveChainRef.current;
  }

  async function run(mode: CoreRunMode) {
    setWorking(mode);
    setError(null);
    setAppliedCoreResults(new Map());
    setCoreApplyErrors(new Map());
    try {
      await waitForPendingPreferenceSaves();
      const preference = clonePreferenceSnapshot(preferenceSnapshotRef.current);
      const [runResult, appPaths] = await Promise.allSettled([
        mode === "check"
          ? checkUpdates(preference.preRelease, preference.selectedIds, true, null)
          : downloadUpdates(preference.preRelease, preference.selectedIds, true, null),
        mode === "check"
          ? checkAppUpdatePaths(preference.preRelease, true, null)
          : loadAppUpdatePaths(preference.preRelease, true, null),
      ]);

      if (runResult.status === "fulfilled") {
        applyUpdateStatus({ preRelease: runResult.value.preRelease, targets: runResult.value.targets });
        setResults(runResult.value.results);
      } else {
        setError(errorMessage(runResult.reason));
      }

      if (appPaths.status === "fulfilled") {
        applyAppUpdatePaths(appPaths.value);
      } else {
        setAppUpdaterError(errorMessage(appPaths.reason));
      }
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setWorking(null);
    }
  }

  async function runAppUpdaterCheck() {
    setWorking("app-check");
    setAppUpdaterError(null);
    try {
      applyAppUpdatePaths(await checkAppUpdatePaths(preferenceSnapshotRef.current.preRelease, true, null));
    } catch (error) {
      setAppUpdaterError(errorMessage(error));
    } finally {
      setWorking(null);
    }
  }

  async function installAppUpdate() {
    setWorking("app-install");
    setAppUpdaterError(null);
    setAppInstallResult(null);
    try {
      setAppInstallResult(await installCheckedAppUpdate());
    } catch (error) {
      setAppUpdaterError(errorMessage(error));
    } finally {
      setWorking(null);
    }
  }

  async function applyCoreUpdate(result: UpdateCheckResult) {
    if (!result.fileName || !result.sha256 || !result.remoteVersion) {
      setError(t("updates.missingDownloadedFields"));
      return;
    }

    setApplyingCoreTargetId(result.targetId);
    setError(null);
    setCoreApplyErrors((current) => withoutMapEntry(current, result.targetId));
    try {
      const applied = await applyDownloadedCoreUpdate({
        targetId: result.targetId,
        fileName: result.fileName,
        sha256: result.sha256,
        remoteVersion: result.remoteVersion,
      });
      setAppliedCoreResults((current) => new Map(current).set(result.targetId, applied));
      setResults((current) => reconcileAppliedCoreResult(current, result.targetId, applied));
      try {
        applyUpdateStatus(await updateStatus());
      } catch (refreshError) {
        setError(errorMessage(refreshError));
      }
    } catch (error) {
      const message = errorMessage(error);
      setCoreApplyErrors((current) => new Map(current).set(result.targetId, message));
      setError(message);
    } finally {
      setApplyingCoreTargetId(null);
    }
  }

  function toggleTarget(id: string, checked: boolean) {
    const nextSelected = new Set(preferenceSnapshotRef.current.selectedIds);
    if (checked) {
      nextSelected.add(id);
    } else {
      nextSelected.delete(id);
    }

    const nextPreference = {
      preRelease: preferenceSnapshotRef.current.preRelease,
      selectedIds: [...nextSelected],
    };
    setPreferenceSnapshot(nextPreference);
    void persistPreference(nextPreference).catch((error: unknown) => setError(errorMessage(error)));
  }

  function togglePreRelease(checked: boolean) {
    const nextPreference = {
      preRelease: checked,
      selectedIds: preferenceSnapshotRef.current.selectedIds,
    };
    setPreferenceSnapshot(nextPreference);
    void persistPreference(nextPreference).catch((error: unknown) => setError(errorMessage(error)));
  }

  return (
    <DialogContent className="sm:max-w-4xl">
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <PackageCheck className="size-4" aria-hidden="true" />
          {t("updates.title")}
        </DialogTitle>
        <DialogDescription className="sr-only">{t("updates.description")}</DialogDescription>
      </DialogHeader>

      <div className="grid gap-4">
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-2">
            <Checkbox
              checked={preRelease}
              disabled={working !== null || applyingCoreTargetId !== null}
              id="updates-pre-release"
              onCheckedChange={(checked) => togglePreRelease(checked === true)}
            />
            <Label className="cursor-pointer text-sm font-normal" htmlFor="updates-pre-release">
              {t("updates.preRelease")}
            </Label>
          </div>
          <div className="ms-auto flex items-center gap-2">
            <Button
              disabled={working !== null || selectedIds.size === 0}
              onClick={() => void run("check")}
              type="button"
              variant="outline"
            >
              <RefreshCw className={cn("size-4", working === "check" && "animate-spin")} aria-hidden="true" />
              {t("updates.check")}
            </Button>
            <Button
              disabled={working !== null || selectedIds.size === 0}
              onClick={() => void run("download")}
              type="button"
            >
              <Download className={cn("size-4", working === "download" && "animate-pulse")} aria-hidden="true" />
              {t("updates.download")}
            </Button>
          </div>
        </div>

        {selectedIds.size === 0 ? (
          <Alert>
            <AlertTriangle aria-hidden="true" />
            <AlertDescription>{t("updates.noSelectedTargets")}</AlertDescription>
          </Alert>
        ) : null}

        {error ? (
          <Alert variant="destructive">
            <AlertTriangle aria-hidden="true" />
            <AlertDescription className="break-words">
              {redactOperationalMessage(error, t)}
            </AlertDescription>
          </Alert>
        ) : null}

        <AppUpdatePanel
          appInstallResult={appInstallResult}
          appUpdaterCheck={appUpdaterCheck}
          appUpdaterError={appUpdaterError}
          appUpdaterStatus={appUpdaterStatus}
          manualLinks={manualLinks}
          manualLinksError={manualLinksError}
          onCheck={() => void runAppUpdaterCheck()}
          onInstall={() => void installAppUpdate()}
          working={working}
        />

        <ScrollArea className="h-[24rem] rounded-md border">
          <Table className="min-w-[42rem]">
            <TableHeader className="sticky top-0 z-10 bg-card">
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-12 px-3" scope="col">
                  <span className="sr-only">{t("updates.selected")}</span>
                </TableHead>
                <TableHead className="px-3" scope="col">
                  {t("updates.target")}
                </TableHead>
                <TableHead className="px-3" scope="col">
                  {t("updates.version")}
                </TableHead>
                <TableHead className="px-3" scope="col">
                  {t("updates.status")}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(status?.targets ?? []).map((target) => {
                const result = resultByTarget.get(target.id);
                const applied = appliedCoreResults.get(target.id) ?? null;
                const applyError = coreApplyErrors.get(target.id) ?? null;
                const canApplyCore =
                  target.kind === "core" && result?.status === "downloaded" && !applied;

                return (
                  <TableRow key={target.id}>
                    <TableCell className="px-3 py-2 align-top">
                      <Checkbox
                        aria-label={`${t("updates.selected")} ${target.name}`}
                        checked={selectedIds.has(target.id)}
                        disabled={working !== null || applyingCoreTargetId !== null}
                        onCheckedChange={(checked) => toggleTarget(target.id, checked === true)}
                      />
                    </TableCell>
                    <TableCell className="min-w-48 whitespace-normal px-3 py-2 align-top">
                      <div className="grid gap-1">
                        <div className="flex flex-wrap items-center gap-1.5">
                          <span className="font-medium">{target.name}</span>
                          <TargetKindBadge kind={target.kind} />
                          {target.kind === "app" ? (
                            <ManualStateBadge manualLinks={manualLinks} manualLinksError={manualLinksError} />
                          ) : null}
                        </div>
                        <div className="flex flex-wrap items-center gap-1.5">
                          <AcquisitionBadge acquisition={target.acquisition} />
                          {target.license ? <Badge variant="outline">{target.license}</Badge> : null}
                        </div>
                        <span className="break-words text-xs text-muted-foreground">{target.remarks}</span>
                      </div>
                    </TableCell>
                    <TableCell className="px-3 py-2 align-top text-muted-foreground">
                      {applied?.appliedVersion ?? result?.remoteVersion ?? result?.currentVersion ?? "-"}
                    </TableCell>
                    <TableCell className="min-w-56 px-3 py-2 align-top">
                      <div className="flex flex-wrap items-start gap-2">
                        <UpdateResultBadge applyError={applyError} applied={applied} result={result} target={target} />
                        {canApplyCore ? (
                          <Button
                            disabled={applyingCoreTargetId !== null}
                            onClick={() => void applyCoreUpdate(result)}
                            size="sm"
                            type="button"
                            variant="outline"
                          >
                            <PackageCheck
                              className={cn(
                                "size-4",
                                applyingCoreTargetId === target.id && "animate-pulse",
                              )}
                              aria-hidden="true"
                            />
                            {t("updates.apply")}
                          </Button>
                        ) : null}
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </ScrollArea>
      </div>

      <DialogFooter />
    </DialogContent>
  );
}

function AppUpdatePanel({
  appInstallResult,
  appUpdaterCheck,
  appUpdaterError,
  appUpdaterStatus,
  manualLinks,
  manualLinksError,
  onCheck,
  onInstall,
  working,
}: {
  appInstallResult: AppUpdateInstallResult | null;
  appUpdaterCheck: AppUpdateCheckResult | null;
  appUpdaterError: string | null;
  appUpdaterStatus: AppUpdaterStatus | null;
  manualLinks: ManualAppUpdateLinks | null;
  manualLinksError: string | null;
  onCheck: () => void;
  onInstall: () => void;
  working: RunMode | null;
}) {
  const { t } = useI18n();
  const update = appUpdaterCheck?.update ?? null;
  const statusMessage = appUpdaterStatus?.message
    ? redactOperationalMessage(appUpdaterStatus.message, t)
    : t("updates.appUpdaterReady");

  return (
    <div className="grid gap-3 rounded-md border p-3">
      <div className="flex flex-wrap items-start gap-3">
        <div className="grid gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-sm font-medium">{t("updates.appUpdater")}</span>
            <Badge variant={appUpdaterStatus?.state === "ready" ? "secondary" : "outline"}>
              {appUpdaterStatus
                ? t(`updates.appUpdaterState.${appUpdaterStatus.state}`)
                : t("updates.waiting")}
            </Badge>
          </div>
          <p className="text-xs text-muted-foreground">
            {update
              ? t("updates.appUpdateAvailable", { version: update.version })
              : appUpdaterCheck
                ? t("updates.noAppUpdate")
                : statusMessage}
          </p>
          {appInstallResult ? (
            <p className="text-xs text-muted-foreground">
              {appInstallResult.state === "installed" && appInstallResult.installedVersion
                ? t("updates.appInstalled", { version: appInstallResult.installedVersion })
                : t("updates.noAppUpdate")}
            </p>
          ) : null}
        </div>
        <div className="ms-auto flex flex-wrap items-center gap-2">
          <Button
            disabled={working !== null}
            onClick={onCheck}
            size="sm"
            type="button"
            variant="outline"
          >
            <RefreshCw
              className={cn("size-4", working === "app-check" && "animate-spin")}
              aria-hidden="true"
            />
            {t("updates.checkApp")}
          </Button>
          <Button
            disabled={working !== null || !update}
            onClick={onInstall}
            size="sm"
            type="button"
          >
            <PackageCheck
              className={cn("size-4", working === "app-install" && "animate-pulse")}
              aria-hidden="true"
            />
            {t("updates.installApp")}
          </Button>
        </div>
      </div>

      {appUpdaterError ? (
        <p className="break-words text-xs text-destructive">
          {redactOperationalMessage(appUpdaterError, t)}
        </p>
      ) : null}

      <div className="grid gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium">{t("updates.manualDownloads")}</span>
          {manualLinks ? (
            <Badge variant={manualLinks.hasUpdate ? "secondary" : "outline"}>
              {manualLinks.hasUpdate
                ? t("updates.manualUpdateAvailable", {
                    version: manualLinks.remoteVersion ?? manualLinks.currentVersion,
                  })
                : t("updates.manualCurrent")}
            </Badge>
          ) : null}
        </div>

        {manualLinks?.downloads.length ? (
          <div className="flex flex-wrap gap-2">
            {manualLinks.downloads.map((download) => (
              <a
                className="inline-flex h-8 max-w-full min-w-0 items-center gap-2 rounded-md border px-3 text-xs font-medium text-foreground hover:bg-accent hover:text-accent-foreground"
                href={download.url}
                key={`${download.kind}:${download.url}`}
                rel="noopener noreferrer"
                target="_blank"
              >
                <ExternalLink className="size-3.5" aria-hidden="true" />
                <span className="truncate">{manualDownloadLabel(download)}</span>
              </a>
            ))}
          </div>
        ) : (
          <p className="break-words text-xs text-muted-foreground">
            {manualLinksError
              ? redactOperationalMessage(manualLinksError, t)
              : t("updates.manualLinksUnavailable")}
          </p>
        )}
      </div>
    </div>
  );
}

function UpdateResultBadge({
  applyError,
  applied,
  result,
  target,
}: {
  applyError: string | null;
  applied: CoreUpdateApplyResult | null;
  result: UpdateCheckResult | undefined;
  target: UpdateTarget;
}) {
  const { t } = useI18n();

  if (!result) {
    return <Badge variant="secondary">{t("updates.waiting")}</Badge>;
  }

  const isFailed = Boolean(applyError) || result.status === "error";
  const tone = isFailed
    ? statusTone("error")
    : applied
      ? statusTone("downloaded")
      : statusTone(result.status);
  const label = updateResultLabel({ applyError, applied, result, target, t });

  return (
    <Badge
      className={cn(
        "max-w-full items-start justify-start gap-2 whitespace-normal px-2 py-1 text-left",
        tone,
      )}
      variant="outline"
    >
      {applied || result.status === "upToDate" || result.status === "downloaded" ? (
        <CheckCircle2 className="mt-0.5 shrink-0" aria-hidden="true" />
      ) : isFailed ? (
        <AlertTriangle className="shrink-0" aria-hidden="true" />
      ) : null}
      <span className="min-w-0 break-words">{label}</span>
    </Badge>
  );
}

function TargetKindBadge({ kind }: { kind: UpdateTargetKind }) {
  const { t } = useI18n();

  return <Badge variant="outline">{t(`updates.kind.${kind}`)}</Badge>;
}

function AcquisitionBadge({ acquisition }: { acquisition: UpdateAcquisition }) {
  const { t } = useI18n();

  return <Badge variant="outline">{t(`updates.acquisition.${acquisition}`)}</Badge>;
}

function ManualStateBadge({
  manualLinks,
  manualLinksError,
}: {
  manualLinks: ManualAppUpdateLinks | null;
  manualLinksError: string | null;
}) {
  const { t } = useI18n();

  if (manualLinks?.hasUpdate) {
    return <Badge variant="secondary">{t("updates.manualState.available")}</Badge>;
  }

  if (manualLinks) {
    return <Badge variant="outline">{t("updates.manualState.current")}</Badge>;
  }

  if (manualLinksError) {
    return <Badge variant="outline">{t("updates.manualState.failed")}</Badge>;
  }

  return <Badge variant="outline">{t("updates.manualState.waiting")}</Badge>;
}

function updateResultLabel({
  applyError,
  applied,
  result,
  target,
  t,
}: {
  applyError: string | null;
  applied: CoreUpdateApplyResult | null;
  result: UpdateCheckResult;
  target: UpdateTarget;
  t: (key: string, options?: Record<string, unknown>) => string;
}) {
  if (applyError) {
    return t("updates.statusFailedMessage", {
      message: redactOperationalMessage(applyError, t),
    });
  }

  if (applied) {
    return t("updates.statusAppliedVersion", { version: applied.appliedVersion });
  }

  switch (result.status) {
    case "downloaded":
      return result.remoteVersion
        ? t("updates.statusDownloadedVersion", { version: result.remoteVersion })
        : t("updates.statusDownloaded");
    case "updateAvailable":
      return result.remoteVersion
        ? t("updates.statusUpdateAvailableVersion", { version: result.remoteVersion })
        : t("updates.statusUpdateAvailable");
    case "upToDate": {
      const version = result.remoteVersion ?? result.currentVersion;

      return version ? t("updates.statusCurrentVersion", { version }) : t("updates.statusCurrent");
    }
    case "skipped":
      return target.selected ? t("updates.statusSkipped") : t("updates.statusNotSelected");
    case "error":
      return t("updates.statusFailedMessage", {
        message: redactOperationalMessage(result.message, t),
      });
  }
}

function manualDownloadLabel(download: ManualAppUpdateDownload) {
  const kind = download.kind.trim();

  return kind ? `${kind.toUpperCase()} ${download.name}` : download.name;
}

function statusTone(status: UpdateResultStatus) {
  switch (status) {
    case "downloaded":
    case "upToDate":
      return "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300";
    case "updateAvailable":
      return "border-primary/30 bg-primary/10 text-primary";
    case "error":
      return "border-destructive/40 bg-destructive/10 text-destructive";
    case "skipped":
      return "border-transparent bg-muted text-muted-foreground";
  }
}

function clonePreferenceSnapshot(snapshot: PreferenceSnapshot): PreferenceSnapshot {
  return {
    preRelease: snapshot.preRelease,
    selectedIds: [...snapshot.selectedIds],
  };
}

function preferenceSnapshotFromStatus(status: UpdateStatus): PreferenceSnapshot {
  return {
    preRelease: status.preRelease,
    selectedIds: status.targets.filter((target) => target.selected).map((target) => target.id),
  };
}

function preferenceSnapshotKey(snapshot: PreferenceSnapshot) {
  return JSON.stringify({
    preRelease: snapshot.preRelease,
    selectedIds: [...new Set(snapshot.selectedIds)].sort(),
  });
}

function reconcileAppliedCoreResult(
  current: UpdateCheckResult[],
  targetId: string,
  applied: CoreUpdateApplyResult,
): UpdateCheckResult[] {
  return current.map((item) =>
    item.targetId === targetId
      ? {
          ...item,
          bytes: null,
          currentVersion: applied.appliedVersion,
          downloadUrl: null,
          fileName: null,
          remoteVersion: applied.appliedVersion,
          sha256: null,
          status: "upToDate",
          usedProxy: null,
        }
      : item,
  );
}

function withoutMapEntry<T>(current: Map<string, T>, key: string) {
  const next = new Map(current);
  next.delete(key);

  return next;
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function redactOperationalMessage(
  message: string,
  t: (key: string, options?: Record<string, unknown>) => string,
) {
  return message
    .replace(
      /\b(proxyUrl|proxy_url|proxy|HTTP_PROXY|HTTPS_PROXY)=\S+/gi,
      (_match, key: string) => `${key}=${t("updates.redactedValue")}`,
    )
    .replace(/\bhttps?:\/\/[^\s<>"')\]]+/gi, t("updates.redactedUrl"));
}
