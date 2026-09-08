import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { useI18n } from "@voya/i18n/use-i18n";
import {
  connectActiveProfile,
  disconnectCore,
  listProfiles,
  restartCore,
  runtimeStatus,
  setActiveProfile,
  setConnectionMode,
  systemProxyStatus,
  tunRequestElevation,
  tunStatus,
  useRuntimeEventStore,
} from "@/ipc";
import type { ConnectionMode, TunStatus } from "@/ipc/bindings";
import { profilesQueryKey } from "@/ipc/query-keys";
import { getErrorMessage } from "@voya/utils/error";
import { useModalStore } from "@/stores/modal-store";
import { useToastStore } from "@/stores/toast-store";

import { deriveConnectionMode, isPacActive } from "./connection-mode";
import { missingCorePayload, runWithElevation } from "./runtime-action";

type RuntimeAction = "connect" | "disconnect" | "restart";
export type Translation = ReturnType<typeof useI18n>["t"];

export type ActiveNodeInfo = {
  delayMs: number | null;
  id: string;
  name: string;
};

/**
 * Runtime controller for the Home screen: connect/disconnect/restart with
 * elevation + missing-core handling, the unified connection-mode switcher,
 * node selection/switching, and the seeded sysproxy/TUN live state.
 */
export function useHomeRuntime(t: Translation) {
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const sysProxy = useRuntimeEventStore((state) => state.sysProxy);
  const setSysProxy = useRuntimeEventStore((state) => state.setSysProxy);
  const tun = useRuntimeEventStore((state) => state.tun);
  const setTun = useRuntimeEventStore((state) => state.setTun);
  const openModal = useModalStore((state) => state.openModal);
  const pushToast = useToastStore((state) => state.pushToast);
  const [pendingAction, setPendingAction] = useState<RuntimeAction | null>(null);
  const [modePending, setModePending] = useState<ConnectionMode | null>(null);
  const [pacPending, setPacPending] = useState(false);
  // Local node selection (blue highlight). Seeded from the persisted active
  // profile; single-clicks move it without touching the backend.
  const [selectedId, setSelectedId] = useState<string | null>(null);
  // Tracks the active profile the selection was last seeded from, so re-seeding
  // only fires when the active profile actually changes.
  const [seededFor, setSeededFor] = useState<string | null>(null);
  // The node whose switch+connect is currently in flight (spinner / re-entry guard).
  const [switchingId, setSwitchingId] = useState<string | null>(null);
  // Shares the ProfilesScreen query cache (same key) so resolving the active
  // node's name here costs no extra fetch and stays in sync after a switch.
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: profilesQueryKey(""),
  });

  // Home owns the connection-mode controls, so it seeds their live OS state
  // into the store on mount. Transient `sysProxyChanged` / `tunChanged` events
  // keep it fresh afterwards.
  useEffect(() => {
    let cancelled = false;

    void systemProxyStatus()
      .then((status) => {
        if (!cancelled) {
          setSysProxy(status);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          pushToast({
            description: getErrorMessage(error),
            severity: "error",
            title: t("status.sysProxyStatusFailed"),
          });
        }
      });
    void tunStatus()
      .then((status) => {
        if (!cancelled) {
          setTun(status);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          pushToast({
            description: getErrorMessage(error),
            severity: "error",
            title: t("status.tunStatusFailed"),
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [pushToast, setSysProxy, setTun, t]);

  const state = coreState?.state ?? "disconnected";
  const connected = state === "connected";
  const inProgress = state === "connecting" || state === "disconnecting";
  const busy = inProgress || pendingAction !== null || switchingId !== null;
  // One guard for both mode-mutating controls: they write the same config
  // transaction, so letting them overlap races two `set_connection_mode` calls.
  const modeBusy = busy || modePending !== null || pacPending;

  const activeProfile = profilesQuery.data?.entries.find((item) => item.isActive) ?? null;
  const activeProfileId = activeProfile?.profile.id ?? null;
  // The green "live" dot follows the node that is actually running, which differs
  // from the persisted-active node only while disconnected.
  const runningId = connected ? (coreState?.activeProfileId ?? null) : null;
  const pacAvailable = sysProxy?.pacAvailable ?? false;
  const connectionMode = deriveConnectionMode(sysProxy, tun);
  const pacActive = isPacActive(sysProxy);
  const tunProviderSummary = tun ? tunProviderLabel(tun, t) : null;

  const runningEntry = runningId
    ? (profilesQuery.data?.entries.find((item) => item.profile.id === runningId) ?? null)
    : null;
  const activeNodeEntry = runningEntry ?? (connected ? activeProfile : null);
  const activeNode: ActiveNodeInfo | null = activeNodeEntry
    ? {
        delayMs:
          activeNodeEntry.metrics.delayMs > 0 ? activeNodeEntry.metrics.delayMs : null,
        id: activeNodeEntry.profile.id,
        name: activeNodeEntry.profile.remarks || activeNodeEntry.profile.id,
      }
    : null;

  // Seed the local selection from the persisted active profile and re-sync it
  // whenever the active profile changes (e.g. after a switch). Adjusting state
  // during render (React's documented pattern) instead of in an effect avoids a
  // cascading-render lint and an extra paint. A single-click only moves
  // `selectedId`, not the active profile, so the selection is never clobbered.
  if (activeProfileId && activeProfileId !== seededFor) {
    setSeededFor(activeProfileId);
    setSelectedId(activeProfileId);
  } else if (
    profilesQuery.data
    && selectedId
    && !profilesQuery.data.entries.some((item) => item.profile.id === selectedId)
  ) {
    // The selected node disappeared — deleted on the Profiles screen, or pruned
    // by a subscription update. Fall back to the persisted active profile
    // (`null` when there is none) so Connect never targets a dead id.
    setSeededFor(activeProfileId);
    setSelectedId(activeProfileId);
  }

  async function runRuntimeAction(action: RuntimeAction) {
    if (busy) {
      return;
    }

    setPendingAction(action);
    try {
      const status = await runWithElevation(() =>
        action === "connect"
          ? connectActiveProfile()
          : action === "disconnect"
            ? disconnectCore()
            : restartCore(),
      );

      setCoreState(status);
    } catch (error) {
      const missingCore = missingCorePayload(error);
      if (missingCore) {
        openModal("missingCore", { missingCore });
      } else {
        pushToast({
          description: getErrorMessage(error),
          severity: "error",
          title: runtimeActionLabel(action, t),
        });
      }
      await refreshRuntimeState();
    } finally {
      setPendingAction(null);
    }
  }

  async function refreshRuntimeState() {
    try {
      const status = await runtimeStatus();
      setCoreState(status);
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.runtimeStatusFailed"),
      });
    }
  }

  // Switch the active profile to `indexId` and apply it: restart the tunnel when
  // already connected, otherwise connect. Drives double-click / Enter and the
  // connect button when its selection differs from the active profile.
  async function switchActiveAndApply(indexId: string) {
    // `busy` covers the in-flight switch, a pending connect/disconnect/restart
    // and the backend-reported connecting/disconnecting states (tray or
    // auto-connect), so a double-click can never race another runtime command.
    if (busy) {
      return;
    }

    setSelectedId(indexId);
    setSwitchingId(indexId);
    const wasConnected = connected;
    try {
      await setActiveProfile(indexId);
      const status = await runWithElevation(() =>
        wasConnected ? restartCore() : connectActiveProfile(),
      );
      setCoreState(status);
    } catch (error) {
      const missingCore = missingCorePayload(error);
      if (missingCore) {
        openModal("missingCore", { missingCore });
      } else {
        pushToast({
          description: getErrorMessage(error),
          severity: "error",
          title: t(wasConnected ? "actions.restart" : "actions.connect"),
        });
        await refreshRuntimeState();
      }
    } finally {
      // `set_active_profile` emits the profiles invalidation that drives the
      // active-node highlight, whether or not the connect that follows it
      // succeeds.
      setSwitchingId(null);
    }
  }

  function handlePrimaryAction() {
    if (connected) {
      void runRuntimeAction("disconnect");

      return;
    }
    // Connect to the locally-selected node. When it differs from the persisted
    // active profile, switch first so connect uses it; otherwise connect directly.
    if (selectedId && selectedId !== activeProfileId) {
      void switchActiveAndApply(selectedId);

      return;
    }
    void runRuntimeAction("connect");
  }

  /**
   * TUN preflight shared by the VPN mode entry: native component + provider
   * path checks, then on-demand elevation (one native prompt, no stored
   * password). Returns false when VPN cannot (or should not) be enabled.
   */
  async function ensureVpnPreconditions(): Promise<boolean> {
    const current = await tunStatus();
    if (current.backend !== "process" && !current.nativeComponentReady) {
      pushToast({
        description: current.lastProviderError ?? t("status.nativeTunnelMissing"),
        severity: "error",
        title: t("status.tunEnableFailed"),
      });
      return false;
    }
    if (current.providerPathMismatch) {
      pushToast({
        description: tunProviderPathMismatchDescription(current, t),
        severity: "error",
        title: t("status.tunEnableFailed"),
      });
      return false;
    }
    if (current.requiresElevation && !current.elevationGranted) {
      const granted = await tunRequestElevation();
      if (!granted.elevationGranted) {
        // User cancelled the native dialog — leave the mode unchanged.
        return false;
      }
    }

    return true;
  }

  /**
   * Re-seed the switcher from authoritative status. The backend already emitted
   * `sysProxyChanged`/`tunChanged` post-commit, so a failure here means the mode
   * change still succeeded — it must never be reported as a mode-change failure.
   */
  async function refreshConnectionModeStatus() {
    try {
      setSysProxy(await systemProxyStatus());
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.sysProxyStatusFailed"),
      });
    }
    try {
      setTun(await tunStatus());
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.tunStatusFailed"),
      });
    }
  }

  async function runConnectionMode(mode: ConnectionMode, pacEnabled: boolean | null = null) {
    // `modeBusy` also covers a pending connect/disconnect/restart: flipping TUN
    // while the core is still starting persists the flag but cannot restart the
    // not-yet-connected core, leaving the UI claiming VPN over a non-TUN core.
    if (modeBusy || (mode === connectionMode && pacEnabled === null)) {
      return;
    }

    setModePending(mode);
    try {
      if (mode === "vpn" && !(await ensureVpnPreconditions())) {
        return;
      }
      await setConnectionMode(mode, pacEnabled);
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.connectionModeChangeFailed"),
      });
      return;
    } finally {
      setModePending(null);
    }

    await refreshConnectionModeStatus();
  }

  async function runPacToggle() {
    if (modeBusy || connectionMode !== "systemProxy") {
      return;
    }
    setPacPending(true);
    try {
      const nextPac = !pacActive;
      await setConnectionMode("systemProxy", nextPac);
      setSysProxy(await systemProxyStatus());
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.connectionModeChangeFailed"),
      });
    } finally {
      setPacPending(false);
    }
  }

  function activateProfile(indexId: string) {
    void switchActiveAndApply(indexId);
  }

  function changeConnectionMode(mode: ConnectionMode) {
    void runConnectionMode(mode);
  }

  function restart() {
    void runRuntimeAction("restart");
  }

  function selectProfile(indexId: string) {
    setSelectedId(indexId);
  }

  function togglePac() {
    void runPacToggle();
  }

  return {
    activateProfile,
    activeNode,
    activeSubscriptionId: activeProfile?.profile.subscriptionId ?? null,
    busy,
    changeConnectionMode,
    connected,
    connectionMode,
    handlePrimaryAction,
    inProgress,
    mainPid: coreState?.mainPid ?? null,
    modeBusy,
    modePending,
    pacActive,
    pacAvailable,
    pacPending,
    profiles: profilesQuery.data?.entries ?? [],
    profilesPending: profilesQuery.isPending,
    restart,
    runningId,
    selectProfile,
    selectedId,
    state,
    switchingId,
    togglePac,
    tunProviderSummary,
  };
}

function runtimeActionLabel(action: RuntimeAction, t: Translation) {
  switch (action) {
    case "connect":
      return t("actions.connect");
    case "disconnect":
      return t("actions.disconnect");
    case "restart":
      return t("actions.restart");
  }
}

function tunProviderLabel(tun: TunStatus, t: Translation) {
  const backend = tunBackendLabel(tun.backend, t);
  const providerState = tunProviderStateLabel(tun.providerState, t);
  if (tun.lastProviderError) {
    return `${backend}: ${providerState}: ${tun.lastProviderError}`;
  }

  return `${backend}: ${providerState}`;
}

function tunProviderPathMismatchDescription(status: TunStatus, t: Translation) {
  return t("status.tunProviderPathMismatch", {
    expected: status.expectedProviderPath ?? "—",
    resolved: status.resolvedProviderPath ?? "—",
  });
}

function tunBackendLabel(backend: TunStatus["backend"], t: Translation) {
  switch (backend) {
    case "macosPacketTunnel":
      return t("status.tunBackendMacos");
    case "windowsService":
      return t("status.tunBackendWindows");
    case "process":
      return t("status.tunBackendProcess");
    case "unsupported":
    default:
      return t("status.tunBackendUnsupported");
  }
}

function tunProviderStateLabel(state: TunStatus["providerState"], t: Translation) {
  switch (state) {
    case "running":
      return t("status.tunProviderRunning");
    case "starting":
      return t("status.tunProviderStarting");
    case "stopped":
      return t("status.tunProviderStopped");
    case "permissionRequired":
      return t("status.tunProviderPermissionRequired");
    case "missingComponent":
      return t("status.tunProviderMissingComponent");
    case "error":
      return t("status.tunProviderError");
    case "notApplicable":
    default:
      return t("status.tunProviderNotApplicable");
  }
}
