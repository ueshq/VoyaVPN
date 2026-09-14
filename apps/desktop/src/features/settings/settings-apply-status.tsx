import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, LoaderCircle, RotateCcw } from "lucide-react";
import { Button } from "@voya/ui/components/button";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { applyPendingSettings, getSettingsApplyStatus } from "@/ipc/commands";
import { queryKeys } from "@/ipc/query-keys";
import { PageSurface } from "@/components/app-shell/page-section";

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
  const coreState = useRuntimeEventStore((state) => state.coreState?.state);
  const query = useQuery({
    queryKey: queryKeys.settingsApply,
    queryFn: getSettingsApplyStatus,
    refetchOnMount: "always",
  });
  const { refetch } = query;
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pending = useRef(false);
  useEffect(() => {
    if (!saving) void refetch();
  }, [saving, coreState, refetch]);
  async function apply() {
    if (pending.current) return;
    pending.current = true;
    setWorking(true);
    setError(null);
    try {
      await applyPendingSettings();
    } catch (cause) {
      setError(getErrorMessage(cause));
    } finally {
      pending.current = false;
      setWorking(false);
      await refetch();
    }
  }
  const status = query.data;
  const needsApply = status?.connected && status.action !== "none";
  if (!needsApply && !working && !error && !query.error) {
    if (!saving && (!saved || failed)) return null;
    return (
      <p
        className="inline-flex shrink-0 items-center gap-2 px-4 text-xs text-muted-foreground"
        role="status"
      >
        {saving ? (
          <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin" />
        ) : (
          <Check aria-hidden="true" className="size-3.5" />
        )}
        {saving
          ? t("settings.saveStatus.saving")
          : t(
              status?.connected
                ? "settings.saveStatus.saved"
                : "settings.saveStatus.savedOffline",
            )}
      </p>
    );
  }

  return (
    <PageSurface className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3">
      {working ? (
        <span className="inline-flex items-center gap-2 text-sm" role="status">
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
          {t("settings.apply.working")}
        </span>
      ) : needsApply ? (
        <span className="text-xs text-muted-foreground">
          {t("settings.apply.pending")}
        </span>
      ) : null}
      {needsApply ? (
        <Button
          className="ms-auto"
          size="sm"
          disabled={saving || working || failed}
          onClick={() => void apply()}
        >
          <RotateCcw aria-hidden="true" className="size-4" />
          {t(
            status.action === "reconnect"
              ? "settings.apply.reconnect"
              : "settings.apply.proxy",
          )}
        </Button>
      ) : null}
      {error || query.error ? (
        <div
          className="flex w-full items-center gap-3 text-sm text-danger"
          role="alert"
        >
          <span className="min-w-0 break-words">
            {error || getErrorMessage(query.error)}
          </span>
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
