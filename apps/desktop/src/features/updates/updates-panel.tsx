import { useState } from "react";
import {
  Database,
  Download,
  LoaderCircle,
  PackageCheck,
  RefreshCw,
} from "lucide-react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@voya/ui/components/alert-dialog";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import type { TranslationFunction, TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { redactOperationalMessage } from "@voya/utils/operational-redaction";

import { DisabledReason } from "@/components/disabled-reason";
import { SettingsGroup } from "@/features/settings/settings-form";
import type { AppUpdaterState } from "@/ipc/bindings";

import type { AppUpdateProgress } from "./app-update-flow";
import {
  useCheckUpdateDialog,
  type CheckUpdateDialogController,
} from "./use-check-update-dialog";

export function UpdatesPanel() {
  const controller = useCheckUpdateDialog();
  const { t } = controller;

  return (
    <div className="grid gap-4">
      <h3 className="sr-only">{t("updates.title")}</h3>

      {/* A build without an update feed has nothing to check, so it shows
          no updater at all rather than a permanent error. */}
      {controller.appUpdaterError ||
      (controller.appUpdaterStatus &&
        controller.appUpdaterStatus.state !== "unconfigured") ? (
        <AppUpdatePanel controller={controller} />
      ) : null}
      <RuleLibraryPanel controller={controller} />
    </div>
  );
}

const APP_UPDATER_STATE_TRANSLATION_KEYS = {
  error: "updates.appUpdaterState.error",
  ready: "updates.appUpdaterState.ready",
  unconfigured: "updates.appUpdaterState.unconfigured",
  unsupported: "updates.appUpdaterState.unsupported",
} as const satisfies Record<AppUpdaterState, TranslationKey>;

function AppUpdatePanel({
  controller,
}: {
  controller: CheckUpdateDialogController;
}) {
  const {
    appInstallResult,
    appUpdaterCheck,
    appUpdaterError,
    appUpdaterStatus,
    installAppUpdate: onInstall,
    installProgress,
    restartApp: onRestart,
    runAppUpdaterCheck: onCheck,
    t,
    working,
  } = controller;
  // An install downloads a whole release and ends in a restart, so it waits
  // for a second click that names the version.
  const [confirmingInstall, setConfirmingInstall] = useState(false);
  const update = appUpdaterCheck?.update ?? null;
  const restartRequired = appInstallResult?.restartRequired ?? false;
  const currentVersion =
    appUpdaterCheck?.currentVersion || appUpdaterStatus?.currentVersion || null;
  const busyReason = working !== null ? t("updates.busyReason") : undefined;
  const statusMessage = appUpdaterStatus?.message
    ? redactUpdateMessage(appUpdaterStatus.message, t)
    : appUpdaterStatus
      ? t("updates.appUpdaterReady")
      : t("updates.waiting");

  return (
    <SettingsGroup title={t("updates.appUpdater")}>
      <div className="flex flex-wrap items-start gap-3">
        <div className="grid min-w-0 flex-1 gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <Badge
              variant={
                appUpdaterStatus?.state === "ready" ? "secondary" : "outline"
              }
            >
              {appUpdaterStatus
                ? t(APP_UPDATER_STATE_TRANSLATION_KEYS[appUpdaterStatus.state])
                : t("updates.waiting")}
            </Badge>
            {currentVersion ? (
              <span className="text-xs text-muted-foreground">
                {t("updates.currentVersion", { version: currentVersion })}
              </span>
            ) : null}
          </div>
          <p className="text-xs text-muted-foreground">
            {update
              ? t("updates.appUpdateAvailable", { version: update.version })
              : appUpdaterCheck
                ? t("updates.noAppUpdate")
                : statusMessage}
          </p>
          {working === "app-install" ? (
            <InstallProgress progress={installProgress} t={t} />
          ) : null}
          {appInstallResult ? (
            <p className="text-xs text-muted-foreground">
              {appInstallResult.installedVersion
                ? t("updates.appInstalled", {
                    version: appInstallResult.installedVersion,
                  })
                : t("updates.noAppUpdate")}
            </p>
          ) : null}
          {restartRequired ? (
            <p className="text-xs text-muted-foreground">
              {t("updates.restartRequired")}
            </p>
          ) : null}
        </div>
        <div className="ms-auto flex flex-wrap items-center gap-2">
          {!restartRequired ? (
            <DisabledReason reason={busyReason}>
              <Button
                disabled={working !== null}
                onClick={() => void onCheck()}
                size="sm"
                type="button"
                variant="outline"
              >
                <RefreshCw
                  className={cn(
                    "size-4",
                    working === "app-check" && "animate-spin",
                  )}
                  aria-hidden="true"
                />
                {t("updates.checkApp")}
              </Button>
            </DisabledReason>
          ) : null}
          {update && !restartRequired ? (
            <DisabledReason reason={busyReason}>
              <Button
                disabled={working !== null}
                onClick={() => setConfirmingInstall(true)}
                size="sm"
                type="button"
              >
                <PackageCheck
                  className={cn(
                    "size-4",
                    working === "app-install" && "animate-pulse",
                  )}
                  aria-hidden="true"
                />
                {t("updates.installApp")}
              </Button>
            </DisabledReason>
          ) : null}
          {restartRequired ? (
            <DisabledReason reason={busyReason}>
              <Button
                disabled={working !== null}
                onClick={() => void onRestart()}
                size="sm"
                type="button"
              >
                <RefreshCw
                  className={cn(
                    "size-4",
                    working === "app-restart" && "animate-spin",
                  )}
                  aria-hidden="true"
                />
                {t("updates.restartApp")}
              </Button>
            </DisabledReason>
          ) : null}
        </div>
      </div>

      {appUpdaterError ? (
        <p className="break-words text-xs text-danger">
          {redactUpdateMessage(appUpdaterError, t)}
        </p>
      ) : null}

      <AlertDialog onOpenChange={setConfirmingInstall} open={confirmingInstall}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("updates.confirmInstallTitle", { version: update?.version ?? "" })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("updates.confirmInstallDescription")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction onClick={() => void onInstall()}>
              {t("updates.confirmInstall")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingsGroup>
  );
}

/** A download without a known size shows a pulsing bar instead of a percentage. */
function InstallProgress({
  progress,
  t,
}: {
  progress: AppUpdateProgress | null;
  t: TranslationFunction;
}) {
  const finished = progress?.finished ?? false;
  const percent =
    progress?.total
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;
  const label = finished
    ? t("updates.installing")
    : percent === null
      ? t("updates.downloadingUnknown")
      : t("updates.downloading", { percent });
  const width = finished ? 100 : percent;

  return (
    <div className="grid gap-1" role="status">
      <span className="text-xs text-muted-foreground">{label}</span>
      <div
        aria-hidden="true"
        className="h-1.5 w-full max-w-64 overflow-hidden rounded-full bg-muted"
      >
        <div
          className={cn(
            "h-full rounded-full bg-primary transition-[width]",
            width === null && "w-1/3 animate-pulse",
          )}
          style={width === null ? undefined : { width: `${width}%` }}
        />
      </div>
    </div>
  );
}

function RuleLibraryPanel({
  controller,
}: {
  controller: CheckUpdateDialogController;
}) {
  const {
    ruleLibraryError: error,
    ruleLibraryFiles: files,
    ruleLibraryUpdatedAt: updatedAt,
    t,
    updateRuleLibrary,
    working,
  } = controller;
  const { language } = useI18n();
  const busy = working === "rule-library";
  const title = t("updates.ruleLibraryTitle");

  return (
    <SettingsGroup title={t("settings.sections.resources")}>
      <section
        className="flex flex-wrap items-start gap-3 p-3"
        aria-label={title}
      >
        <Database
          className="mt-0.5 size-4 text-muted-foreground"
          aria-hidden="true"
        />
        <div className="grid min-w-0 flex-1 gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-sm font-medium">{title}</span>
            {files?.length ? (
              <Badge variant="secondary">
                {t("updates.resourceUpdated", { count: files.length })}
              </Badge>
            ) : null}
          </div>
          <p className="text-xs text-muted-foreground">
            {t("updates.ruleLibraryDescription")}
          </p>
          <p className="text-xs text-muted-foreground">
            {updatedAt === null
              ? t("updates.neverUpdated")
              : t("updates.lastUpdated", {
                  time: new Date(updatedAt).toLocaleString(language),
                })}
          </p>
          {files?.length ? (
            <p className="break-words text-xs text-muted-foreground">
              {files.map((file) => file.name).join(", ")}
            </p>
          ) : null}
          {error ? (
            <p className="break-words text-xs text-danger">
              {redactUpdateMessage(error, t)}
            </p>
          ) : null}
        </div>
        <DisabledReason
          reason={working !== null ? t("updates.busyReason") : undefined}
        >
          <Button
            disabled={working !== null}
            onClick={() => void updateRuleLibrary()}
            size="sm"
            type="button"
            variant="outline"
          >
            {busy ? (
              <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <Download className="size-4" aria-hidden="true" />
            )}
            {error ? t("settings.autosave.retry") : t("updates.updateNow")}
          </Button>
        </DisabledReason>
      </section>
    </SettingsGroup>
  );
}

function redactUpdateMessage(message: string, t: TranslationFunction) {
  return redactOperationalMessage(message, {
    redactedUrl: t("updates.redactedUrl"),
    redactedValue: t("updates.redactedValue"),
  });
}
