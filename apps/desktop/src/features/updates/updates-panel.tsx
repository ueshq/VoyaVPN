import { SettingsGroup } from "@/features/settings/settings-form";
import {
  Database,
  Download,
  LoaderCircle,
  PackageCheck,
  RefreshCw,
} from "lucide-react";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import type { TranslationFunction, TranslationKey } from "@voya/i18n";
import { redactOperationalMessage } from "@voya/utils/operational-redaction";
import type { AppUpdaterState } from "@/ipc/bindings";
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

      <AppUpdatePanel controller={controller} />
      <ResourceUpdatePanel controller={controller} />
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
    restartApp: onRestart,
    runAppUpdaterCheck: onCheck,
    t,
    working,
  } = controller;
  const update = appUpdaterCheck?.update ?? null;
  const statusMessage = appUpdaterStatus?.message
    ? redactUpdateMessage(appUpdaterStatus.message, t)
    : appUpdaterStatus
      ? t("updates.appUpdaterReady")
      : t("updates.waiting");

  return (
    <SettingsGroup title={t("updates.appUpdater")}>
      <div className="flex flex-wrap items-start gap-3">
        <div className="grid gap-1">
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
              {appInstallResult.state === "installed" &&
              appInstallResult.installedVersion
                ? t("updates.appInstalled", {
                    version: appInstallResult.installedVersion,
                  })
                : t("updates.noAppUpdate")}
            </p>
          ) : null}
          {appInstallResult?.restartRequired ? (
            <p className="text-xs text-muted-foreground">
              {t("updates.restartRequired")}
            </p>
          ) : null}
        </div>
        <div className="ms-auto flex flex-wrap items-center gap-2">
          {!appInstallResult?.restartRequired ? (
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
          ) : null}
          {update && !appInstallResult?.restartRequired ? (
            <Button
              disabled={working !== null || !update}
              onClick={() => void onInstall()}
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
          ) : null}
          {appInstallResult?.restartRequired ? (
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
          ) : null}
        </div>
      </div>

      {appUpdaterError ? (
        <p className="break-words text-xs text-danger">
          {redactUpdateMessage(appUpdaterError, t)}
        </p>
      ) : null}
    </SettingsGroup>
  );
}

function ResourceUpdatePanel({
  controller,
}: {
  controller: CheckUpdateDialogController;
}) {
  return (
    <SettingsGroup title={controller.t("settings.sections.resources")}>
      <div className="grid divide-y">
        <ResourceRow controller={controller} kind="geo" />
        <ResourceRow controller={controller} kind="srs" />
      </div>
    </SettingsGroup>
  );
}

function ResourceRow({
  controller,
  kind,
}: {
  controller: CheckUpdateDialogController;
  kind: "geo" | "srs";
}) {
  const { resourceErrors, resourceResults, t, updateResource, working } =
    controller;
  const result = resourceResults[kind];
  const error = resourceErrors[kind];
  const busy = working === kind;
  const title = kind === "geo" ? t("updates.geoTitle") : t("updates.srsTitle");
  const description =
    kind === "geo" ? t("updates.geoDescription") : t("updates.srsDescription");
  const Icon = kind === "geo" ? Database : Download;

  return (
    <section
      className="flex flex-wrap items-start gap-3 p-3"
      aria-label={title}
    >
      <Icon
        className="mt-0.5 size-4 text-muted-foreground"
        aria-hidden="true"
      />
      <div className="grid min-w-0 flex-1 gap-1">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm font-medium">{title}</span>
          {result ? (
            <Badge variant="secondary">
              {t("updates.resourceUpdated", { count: result.length })}
            </Badge>
          ) : null}
        </div>
        <p className="text-xs text-muted-foreground">{description}</p>
        {result?.length ? (
          <p className="break-words text-xs text-muted-foreground">
            {result.map((file) => file.name).join(", ")}
          </p>
        ) : null}
        {error ? (
          <p className="break-words text-xs text-danger">
            {redactUpdateMessage(error, t)}
          </p>
        ) : null}
      </div>
      <Button
        disabled={working !== null}
        onClick={() => void updateResource(kind)}
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
    </section>
  );
}

function redactUpdateMessage(message: string, t: TranslationFunction) {
  return redactOperationalMessage(message, {
    redactedUrl: t("updates.redactedUrl"),
    redactedValue: t("updates.redactedValue"),
  });
}
