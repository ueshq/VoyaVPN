import { useId } from "react";

import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { cn } from "@voya/ui/lib/utils";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import type { ConnectionMode } from "@/ipc/bindings";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";

import { SettingsGroup } from "./settings-form";
import { useCaptureMode } from "./use-capture-mode";

const OPTIONS = [
  {
    hintKey: "settings.captureMode.vpnHint",
    labelKey: "settings.captureMode.vpn",
    value: "vpn",
  },
  {
    hintKey: "settings.captureMode.systemProxyHint",
    labelKey: "settings.captureMode.systemProxy",
    value: "systemProxy",
  },
] as const satisfies ReadonlyArray<{
  hintKey: TranslationKey;
  labelKey: TranslationKey;
  value: ConnectionMode;
}>;

/**
 * The Windows and Linux choice between the platform VPN and the system proxy.
 * It is independent of the traffic mode on Home, which decides where captured
 * traffic goes.
 */
export function CaptureModeSetting() {
  const { t } = useI18n();
  const id = useId();
  const capture = useCaptureMode(t);
  // The saved choice above, and here what the running connection really does.
  const statusKey = useRuntimeEventStore((state): TranslationKey => {
    if (state.coreState?.state !== "connected")
      return "settings.captureMode.status.disconnected";
    if (state.tun?.enabled)
      return state.coreState.activeTunBackend
        ? "settings.captureMode.status.vpnActive"
        : "settings.captureMode.status.vpnInactive";
    return state.sysProxy?.effectiveMode === "forcedChange"
      ? "settings.captureMode.status.proxyActive"
      : "settings.captureMode.status.proxyInactive";
  });
  if (!capture.available) return null;

  return (
    <SettingsGroup title={t("settings.sections.captureMode")}>
      <div
        aria-label={t("settings.sections.captureMode")}
        className="grid gap-2 @min-[42rem]:grid-cols-2"
        role="group"
      >
        {OPTIONS.map(({ hintKey, labelKey, value }) => {
          const selected = capture.mode === value;
          const hintId = `${id}-${value}`;
          return (
            <button
              aria-describedby={hintId}
              aria-label={t(labelKey)}
              aria-pressed={selected}
              className={cn(
                "grid gap-0.5 rounded-md border px-3 py-2.5 text-start outline-none transition-colors hover:bg-surface-hovered focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60",
                selected && "border-primary bg-accent-blue-light",
              )}
              disabled={capture.busy}
              key={value}
              onClick={() => void capture.selectMode(value)}
              type="button"
            >
              <span className="text-sm font-medium">{t(labelKey)}</span>
              <span className="text-xs text-muted-foreground" id={hintId}>
                {t(hintKey)}
              </span>
            </button>
          );
        })}
      </div>
      <p className="text-xs text-muted-foreground" role="status">
        {t(statusKey)}
      </p>
      {capture.error ? <InlinePageError>{capture.error}</InlinePageError> : null}
    </SettingsGroup>
  );
}
