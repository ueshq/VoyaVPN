import { queries } from "@voya/client/queries";
import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import { Button } from "@voya/ui/components/button";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useShellStore } from "@/stores/shell-store";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useSavedTrafficMode } from "@voya/features/routing/use-traffic-mode";

/** Capture uses runtime evidence. Routing is explicitly a saved setting: IPC has no live read. */
export function HomeModeSummary() {
  const { t } = useI18n();
  const core = useRuntimeEventStore((state) => state.coreState);
  const tun = useRuntimeEventStore((state) => state.tun);
  const proxy = useRuntimeEventStore((state) => state.sysProxy);
  const pending = useRuntimeActionStore((state) => state.modePending);
  const traffic = useSavedTrafficMode();
  const connected = core?.state === "connected";
  const apply = useQuery({
    ...queries.settingsApply,
    enabled: connected,
    refetchOnMount: "always",
  });
  const { refetch } = apply;
  useEffect(() => {
    if (connected) void refetch();
  }, [connected, core?.mainPid, refetch]);
  const captureKey: TranslationKey = connected
    ? core.activeTunBackend
      ? "settings.captureMode.vpn"
      : proxy?.effectiveMode === "forcedChange"
        ? "settings.captureMode.systemProxy"
        : "home.mode.captureInactive"
    : tun?.enabled ? "settings.captureMode.vpn" : "settings.captureMode.systemProxy";

  return (
    <div className="home-mode-summary">
      {core && tun && proxy ? <Button
        className="h-auto min-h-8 whitespace-normal"
        size="sm" variant="ghost"
        onClick={() => useShellStore.getState().openSettings("connection")}
      >{t(connected ? "home.mode.current" : "home.mode.next", { mode: t(captureKey) })}</Button> : null}
      <Button className="home-mode-chip h-auto min-h-8 whitespace-normal" size="sm" variant="ghost" onClick={() => useShellStore.getState().setActiveTab("rules", true)}>
        {traffic.mode
          ? t("home.mode.routing", { mode: t(traffic.mode === "global" ? "home.globalModeChip" : "home.mode.rule") })
          : t("panes.routing.trafficModeUnavailable")}
      </Button>
      {pending || (connected && apply.data?.action !== undefined && apply.data.action !== "none") ? (
        <p className="w-full text-center text-xs text-warning" role="status">{t(pending ? "home.modePendingReason" : "settings.apply.pending")}</p>
      ) : null}
    </div>
  );
}
