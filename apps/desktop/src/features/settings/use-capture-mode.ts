import { useState } from "react";

import type { TranslationFunction } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { useRuntimeBusy } from "@/stores/runtime-action";
import {
  tunProviderErrorDescription,
  tunProviderPathMismatchDescription,
} from "@/components/app-shell/tun-provider-text";
import type { ConnectionMode } from "@/ipc/bindings";
import { setConnectionMode, tunRequestElevation, tunStatus } from "@/ipc/commands";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { refreshRuntimeStatusAndReport } from "@/ipc/runtime-status";
import { runtimeActionPending, useRuntimeActionStore } from "@voya/client/runtime-action-store";

/**
 * How traffic is captured on Windows and Linux: the platform VPN or the system
 * proxy. macOS only has its PacketTunnel VPN, so there is nothing to choose.
 * A refusal stays next to the control rather than in a passing toast.
 */
export function useCaptureMode() {
  const { t } = useI18n();
  const available = useRuntimeEventStore(
    (state) => state.sysProxy?.management === "automatic",
  );
  const mode: ConnectionMode = useRuntimeEventStore((state) =>
    state.tun?.enabled ? "vpn" : "systemProxy",
  );
  const modePending = useRuntimeActionStore((state) => state.modePending);
  const [error, setError] = useState<string | null>(null);
  // A pending connect blocks the switch too: flipping TUN while the core is
  // still starting persists the flag but cannot restart a core that is not
  // connected yet, leaving the UI claiming TUN over a non-TUN core.
  const busy = useRuntimeBusy();

  async function selectMode(next: ConnectionMode) {
    if (busy || runtimeActionPending() || next === mode) return;
    setError(null);
    const store = useRuntimeActionStore.getState();
    store.setModePending(true);
    try {
      const blocked = next === "vpn" ? await vpnPreflight(t) : null;
      if (blocked) {
        setError(blocked);
        return;
      }
      await setConnectionMode(next);
    } catch (cause) {
      setError(getErrorMessage(cause));
    } finally {
      try {
        await refreshRuntimeStatusAndReport(t);
      } finally {
        store.setModePending(false);
      }
    }
  }

  return { available, busy, error, mode, pending: modePending, selectMode };
}

/**
 * The native component and provider path checks, then the one on-demand
 * authorization prompt. Returns why VPN mode cannot be entered, or `null`.
 */
async function vpnPreflight(t: TranslationFunction): Promise<string | null> {
  const current = await tunStatus();
  if (current.backend !== "process" && !current.nativeComponentReady) {
    return tunProviderErrorDescription(current, t) ?? t("status.nativeTunnelMissing");
  }
  if (current.providerPathMismatch) {
    return tunProviderPathMismatchDescription(current, t);
  }
  if (current.requiresElevation && !current.elevationGranted) {
    const granted = await tunRequestElevation();
    if (!granted.elevationGranted) {
      return t("settings.captureMode.authorizationDeclined");
    }
  }
  return null;
}
