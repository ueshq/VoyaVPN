import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import type { ConnectionMode } from "@voya/contracts";
import { Button } from "@voya/ui/components/button";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useShellStore } from "@/stores/shell-store";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useSavedTrafficMode } from "@voya/features/routing/use-traffic-mode";
import { activeCaptureMode, savedCaptureMode } from "@/features/settings/use-capture-mode";
import { useSettingsApplyStatus } from "@voya/features/settings/use-settings-apply-status";
import { runningConnectionKey } from "@voya/features/home/use-connection-ip";

const CAPTURE_MODE_KEYS = {
  systemProxy: "settings.captureMode.systemProxy",
  vpn: "settings.captureMode.vpn",
} as const satisfies Record<ConnectionMode, TranslationKey>;

/** Capture uses runtime evidence. Routing is explicitly a saved setting: IPC has no live read. */
export function HomeModeSummary() {
  const { t } = useI18n();
  const core = useRuntimeEventStore((state) => state.coreState);
  // Whether both have reported yet; what they say is read by the selectors below.
  const captureKnown = useRuntimeEventStore((state) => state.tun !== null && state.sysProxy !== null);
  const pending = useRuntimeActionStore((state) => state.modePending);
  const traffic = useSavedTrafficMode();
  const connected = core?.state === "connected";
  // Each connection is a new answer.
  const apply = useSettingsApplyStatus({
    enabled: connected,
    refreshKey: useRuntimeEventStore(runningConnectionKey),
  });
  const active = useRuntimeEventStore(activeCaptureMode);
  const saved = useRuntimeEventStore(savedCaptureMode);
  const captureKey: TranslationKey =
    active === "inactive" ? "home.mode.captureInactive" : CAPTURE_MODE_KEYS[active ?? saved];

  return (
    <div className="home-mode-summary">
      {core && captureKnown ? (
        <Button
          className="h-auto min-h-8 whitespace-normal"
          size="sm"
          variant="ghost"
          onClick={() => useShellStore.getState().openSettings("connection")}
        >
          {t(connected ? "home.mode.current" : "home.mode.next", { mode: t(captureKey) })}
        </Button>
      ) : null}
      <Button
        className="home-mode-chip h-auto min-h-8 whitespace-normal"
        size="sm"
        variant="ghost"
        onClick={() => useShellStore.getState().setActiveTab("rules", true)}
      >
        {traffic.mode
          ? t("home.mode.routing", { mode: t(traffic.mode === "global" ? "home.globalModeChip" : "home.mode.rule") })
          : t("panes.routing.trafficModeUnavailable")}
      </Button>
      {pending || (connected && apply.data?.action !== undefined && apply.data.action !== "none") ? (
        <p className="w-full text-center text-xs text-warning" role="status">
          {t(pending ? "home.modePendingReason" : "settings.apply.pending")}
        </p>
      ) : null}
    </div>
  );
}
