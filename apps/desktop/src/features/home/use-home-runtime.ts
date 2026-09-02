import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

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
import type { ConnectionMode, TunChanged, TunStatus } from "@/ipc/bindings";
import { getErrorMessage } from "@voya/utils/error";
import { useModalStore } from "@/stores/modal-store";
import { useToastStore } from "@/stores/toast-store";

import { deriveConnectionMode, isPacActive } from "./connection-mode";
import {
  missingCorePayload,
  runWithElevation,
  statusToCoreState,
  statusToSysProxyChanged,
  statusToTunChanged,
} from "./runtime-action";

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
  const queryClient = useQueryClient();
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
    queryKey: ["profiles", { filter: "" }],
  });

  // Home owns the connection-mode controls, so it seeds their live OS state
  // into the store on mount. Transient `sysProxyChanged` / `tunChanged` events
  // keep it fresh afterwards.
  useEffect(() => {
    let cancelled = false;

    void systemProxyStatus()
      .then((status) => {
        if (!cancelled) {
          setSysProxy(statusToSysProxyChanged(status));
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
          setTun(statusToTunChanged(status));
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

  const activeProfile = profilesQuery.data?.find((item) => item.isActive) ?? null;
  const activeProfileId = activeProfile?.profile.id ?? null;
  // The green "live" dot follows the node that is actually running, which differs
  // from the persisted-active node only while disconnected.
  const runningId = connected ? (coreState?.activeProfileId ?? null) : null;
  const pacAvailable = sysProxy?.pacAvailable ?? false;
  const connectionMode = deriveConnectionMode(sysProxy, tun);
  const pacActive = isPacActive(sysProxy);
  const tunProviderSummary = tun ? tunProviderLabel(tun, t) : null;

  const runningEntry = runningId
    ? (profilesQuery.data?.find((item) => item.profile.id === runningId) ?? null)
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
  }

  async function runRuntimeAction(action: RuntimeAction) {
    setPendingAction(action);
    try {
      const status = await runWithElevation(() =>
        action === "connect"
          ? connectActiveProfile()
          : action === "disconnect"
            ? disconnectCore()
            : restartCore(),
      );

      setCoreState(statusToCoreState(status));
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
      setCoreState(statusToCoreState(status));
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
    if (switchingId !== null) {
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
      setCoreState(statusToCoreState(status));
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
      // The active profile changed in the DB regardless of connect success, so
      // refresh the cache that drives the active-node highlight.
      await queryClient.invalidateQueries({ queryKey: ["profiles"] });
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

  async function runConnectionMode(mode: ConnectionMode, pacEnabled: boolean | null = null) {
    if (modePending !== null || (mode === connectionMode && pacEnabled === null)) {
      return;
    }

    setModePending(mode);
    try {
      if (mode === "vpn" && !(await ensureVpnPreconditions())) {
        return;
      }
      await setConnectionMode(mode, pacEnabled);
      // The backend also emits sysProxyChanged/tunChanged post-commit; seeding
      // from fresh status keeps the switcher exact without waiting on events.
      setSysProxy(statusToSysProxyChanged(await systemProxyStatus()));
      setTun(statusToTunChanged(await tunStatus()));
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.connectionModeChangeFailed"),
      });
    } finally {
      setModePending(null);
    }
  }

  async function runPacToggle() {
    if (pacPending || connectionMode !== "systemProxy") {
      return;
    }
    setPacPending(true);
    try {
      const nextPac = !pacActive;
      await setConnectionMode("systemProxy", nextPac);
      setSysProxy(statusToSysProxyChanged(await systemProxyStatus()));
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
    modePending,
    pacActive,
    pacAvailable,
    pacPending,
    profiles: profilesQuery.data ?? [],
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

function tunProviderLabel(tun: TunChanged, t: Translation) {
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

function tunBackendLabel(backend: TunChanged["backend"], t: Translation) {
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

function tunProviderStateLabel(state: TunChanged["providerState"], t: Translation) {
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
