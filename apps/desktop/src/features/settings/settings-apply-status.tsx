import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Check, LoaderCircle, RotateCcw } from "lucide-react";
import { Button } from "@voya/ui/components/button";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { applyPendingSettings, getSettingsApplyStatus } from "@/ipc/commands";
import { queryKeys } from "@/ipc/query-keys";

export function SettingsApplyStatus({
  saving,
  failed,
}: {
  saving: boolean;
  failed: boolean;
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
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-b bg-surface-raised px-4 py-3 min-[1100px]:px-page">
      <span className="inline-flex items-center gap-2 text-sm" role="status">
        {saving || working ? (
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
        ) : failed ? null : (
          <Check aria-hidden="true" className="size-4 text-success" />
        )}
        {working
          ? t("settings.apply.working")
          : saving
            ? t("settings.autosave.saving")
            : failed
              ? t("settings.autosave.failed")
              : t("settings.autosave.saved")}
      </span>
      {status ? (
        <span className="text-xs text-muted-foreground">
          {!status.connected
            ? t("settings.apply.nextConnection")
            : status.action === "none"
              ? t("settings.apply.current")
              : t("settings.apply.pending")}
        </span>
      ) : null}
      {status?.connected && status.action !== "none" ? (
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
    </div>
  );
}
