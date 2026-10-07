import { redactOperationalError } from "@voya/utils/operational-redaction";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { Check, RotateCcw } from "lucide-react";
import { Button } from "@voya/ui/components/button";
import { Spinner } from "@voya/ui/components/spinner";
import { useI18n } from "@voya/i18n/use-i18n";
import { voyaCommands } from "@voya/client/transport";
import { PageSurface } from "@/components/app-shell/page-section";
import { useDialogSubmit } from "@/lib/use-dialog-submit";

import { runningConnectionKey } from "@voya/features/home/use-connection-ip";
import { useSettingsApplyStatus } from "@voya/features/settings/use-settings-apply-status";

/**
 * Where the automatic saves stand: saving, saved, or saved but waiting to be
 * applied to the running connection.
 */
export function SettingsApplyStatus({
  saving,
  failed,
  saved = false,
}: {
  saving: boolean;
  failed: boolean;
  saved?: boolean;
}) {
  const { t } = useI18n();
  // A save, a new connection or the end of one is what makes the status
  // stale — not each step a connect passes through on the way. While a save
  // is still running the answer would describe the settings before it.
  const connection = useRuntimeEventStore(runningConnectionKey);
  const query = useSettingsApplyStatus({
    refreshKey: saving ? null : (connection ?? ""),
  });
  const { refetch } = query;
  const { error: applyError, pending: working, submit } = useDialogSubmit();
  // Both buttons that reach this are disabled while it runs. Whether it
  // worked or not, the status is read again: a failed apply may have applied
  // part of what was pending.
  async function apply() {
    await submit(async () => {
      await voyaCommands().applyPendingSettings();
    });
    await refetch();
  }
  const status = query.data;
  const needsApply = status?.connected && status.action !== "none";
  // A failed apply is only worth showing while there is still something to
  // apply: once the core reconnected or went away, retrying it sends a command
  // with nothing pending.
  const error = needsApply ? applyError : null;
  if (!needsApply && !working && !error && !query.error) {
    if (!saving && (!saved || failed)) return null;
    return (
      <p className="inline-flex shrink-0 items-center gap-2 px-4 text-xs text-muted-foreground" role="status">
        {saving ? <Spinner className="size-3.5" /> : <Check aria-hidden="true" className="size-3.5" />}
        {saving
          ? t("settings.saveStatus.saving")
          : t(status?.connected ? "settings.saveStatus.saved" : "settings.saveStatus.savedOffline")}
      </p>
    );
  }

  return (
    <PageSurface className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
      {working ? (
        <span className="inline-flex items-center gap-2 text-sm" role="status">
          <Spinner className="size-4" />
          {t("settings.apply.working")}
        </span>
      ) : needsApply ? (
        <span className="text-xs text-muted-foreground">{t("settings.apply.pending")}</span>
      ) : null}
      {needsApply ? (
        <Button className="ms-auto" size="sm" disabled={saving || working || failed} onClick={() => void apply()}>
          <RotateCcw aria-hidden="true" className="size-4" />
          {t(status.action === "reconnect" ? "settings.apply.reconnect" : "settings.apply.proxy")}
        </Button>
      ) : null}
      {error || query.error ? (
        <div className="flex w-full items-center gap-3 text-sm text-danger" role="alert">
          <span className="min-w-0 break-words">{error || redactOperationalError(query.error)}</span>
          <Button
            size="sm"
            variant="outline"
            disabled={working || saving}
            onClick={() => void (error ? apply() : refetch())}
          >
            {t("settings.autosave.retry")}
          </Button>
        </div>
      ) : null}
    </PageSurface>
  );
}
