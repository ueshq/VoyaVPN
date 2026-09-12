import { useState } from "react";
import { Button } from "@voya/ui/components/button";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { useMountedRef } from "@voya/utils/use-mounted-ref";
import { openNetworkSettings, recheckSystemProxy } from "@/ipc/commands";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { beginRuntimeRead } from "@/ipc/runtime-state-version";
import type { SystemProxyStatusResponse } from "@/ipc/bindings";

export function ManualProxyPanel({ status, connected, tunEnabled }: {
  status: SystemProxyStatusResponse;
  connected: boolean;
  tunEnabled: boolean;
}) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const mounted = useMountedRef();
  const setSysProxy = useRuntimeEventStore((state) => state.setSysProxy);
  const endpoint = connected && !tunEnabled ? status.proxy : null;
  const pacUrl = connected && !tunEnabled ? status.pacUrl : null;
  const address = status.requestedMode === "pac" ? pacUrl : endpoint;
  const observation = status.observation === "clear" ? t("home.manualProxy.clear")
    : status.observation === "localProxy" ? t("home.manualProxy.localProxy")
      : status.observation === "otherProxy" ? t("home.manualProxy.otherProxy")
        : t("home.manualProxy.unknown");

  async function run(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError(null);
    setCopied(false);
    try {
      await action();
    } catch (failure) {
      if (mounted.current) setError(getErrorMessage(failure));
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  return (
    <div className="grid w-full gap-2 rounded-lg bg-muted/30 p-3 text-sm" data-testid="manual-proxy-panel">
      <p className="font-medium">{t("settings.network.manualProxySetup")}</p>
      <p className="text-muted-foreground">{t("home.manualProxy.instructions")}</p>
      <p role="status">{observation}</p>
      {address ? (
        <p className="break-all font-mono text-xs">
          {pacUrl && status.requestedMode === "pac"
            ? t("home.manualProxy.pac", { address })
            : t("home.manualProxy.endpoint", { address })}
        </p>
      ) : <p className="text-muted-foreground">{t("home.manualProxy.inactive")}</p>}
      {endpoint && status.exceptions ? <p className="break-words text-xs">{t("home.manualProxy.bypass", { exceptions: status.exceptions })}</p> : null}
      {status.manualCleanupRequired || tunEnabled ? (
        <p className="text-amber-700 dark:text-amber-400">{t("home.manualProxy.cleanup")}</p>
      ) : <p className="text-xs text-muted-foreground">{t("home.manualProxy.cleanup")}</p>}
      <div className="flex flex-wrap gap-2">
        <Button disabled={busy} onClick={() => void run(openNetworkSettings)} size="sm" variant="outline">
          {t("home.manualProxy.openSettings")}
        </Button>
        <Button disabled={busy} onClick={() => void run(async () => {
          const isLatest = beginRuntimeRead("sysProxy");
          const next = await recheckSystemProxy();
          if (mounted.current && isLatest()) setSysProxy(next);
        })} size="sm" variant="outline">
          {t("home.manualProxy.recheck")}
        </Button>
        {address ? <Button disabled={busy} onClick={() => void run(async () => {
          await navigator.clipboard.writeText(address);
          if (mounted.current) setCopied(true);
        })} size="sm" variant="outline">{t("home.manualProxy.copy")}</Button> : null}
      </div>
      {copied ? <p role="status">{t("home.manualProxy.copied")}</p> : null}
      {error ? <p role="alert" className="break-words text-danger">{error}</p> : null}
    </div>
  );
}
