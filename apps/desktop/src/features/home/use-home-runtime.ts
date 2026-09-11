import { useQuery } from "@tanstack/react-query";

import { useI18n } from "@voya/i18n/use-i18n";
import {
  connectActiveProfile,
  disconnectCore,
  listProfiles,
  restartCore,
  setConnectionMode,
  tunRequestElevation,
  tunStatus,
} from "@/ipc/commands";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { TunStatus } from "@/ipc/bindings";
import {
  refreshRuntimeStatus,
  runtimeStatusErrorKeys,
} from "@/ipc/runtime-status";
import { beginRuntimeRead } from "@/ipc/runtime-state-version";
import { profilesQueryKey } from "@/ipc/query-keys";
import { getErrorMessage } from "@voya/utils/error";
import {
  runtimeActionPending,
  useRuntimeActionStore,
} from "@/stores/runtime-action-store";
import { useModalStore } from "@/stores/modal-store";
import { useToastStore } from "@/stores/toast-store";

import { missingCorePayload, runWithElevation } from "./runtime-action";

type RuntimeAction = "connect" | "disconnect" | "restart";
export type Translation = ReturnType<typeof useI18n>["t"];

/**
 * Runtime controller for the Home screen: connect/disconnect/restart with
 * elevation + missing-core handling, the unified connection-mode switcher,
 * node selection/switching, and the seeded sysproxy/TUN live state.
 */
export function useHomeRuntime(t: Translation) {
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const sysProxy = useRuntimeEventStore((state) => state.sysProxy);
  const tun = useRuntimeEventStore((state) => state.tun);
  const openModal = useModalStore((state) => state.openModal);
  const pushToast = useToastStore((state) => state.pushToast);
  const pendingAction = useRuntimeActionStore((state) => state.pendingAction);
  const modePending = useRuntimeActionStore((state) => state.modePending);
  const switchingId = useRuntimeActionStore((state) => state.switchingId);
  // Shares the ProfilesScreen query cache (same key) so resolving the active
  // node's name here costs no extra fetch and stays in sync after a switch.
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: profilesQueryKey(""),
  });

  const state = coreState?.state ?? "disconnected";
  const connected = state === "connected";
  const inProgress = state === "connecting" || state === "disconnecting";
  const busy =
    inProgress || pendingAction !== null || switchingId !== null || modePending;
  // TUN and traffic mode both update the connection configuration, so their
  // commands share one guard with connect/disconnect and profile switching.
  const modeBusy = busy;

  const activeProfile =
    profilesQuery.data?.entries.find((item) => item.isActive) ?? null;
  // The green "live" dot follows the node that is actually running, which differs
  // from the persisted-active node only while disconnected.
  const runningId = connected ? (coreState?.activeProfileId ?? null) : null;
  const tunEnabled = tun?.enabled ?? false;
  const tunProviderSummary = tun ? tunProviderLabel(tun, t) : null;

  const runningEntry = runningId
    ? (profilesQuery.data?.entries.find(
        (item) => item.profile.id === runningId,
      ) ?? null)
    : null;
  const nodeEntry = connected ? runningEntry : activeProfile;

  async function runRuntimeAction(action: RuntimeAction) {
    if (busy || runtimeActionPending()) {
      return;
    }

    useRuntimeActionStore.setState({ pendingAction: action });
    const isLatest = beginRuntimeRead("coreState");
    try {
      const status = await runWithElevation(() =>
        action === "connect"
          ? connectActiveProfile()
          : action === "disconnect"
            ? disconnectCore()
            : restartCore(),
      );

      if (isLatest()) setCoreState(status);
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
    } finally {
      await refreshStatus();
      useRuntimeActionStore.setState({ pendingAction: null });
    }
  }

  async function refreshStatus(
    channels?: Parameters<typeof refreshRuntimeStatus>[0],
  ) {
    const failures = await refreshRuntimeStatus(channels);
    for (const { channel, error } of failures) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t(runtimeStatusErrorKeys[channel]),
      });
    }
  }

  function handlePrimaryAction() {
    if (connected || state === "cleanupPending") {
      void runRuntimeAction("disconnect");

      return;
    }
    void runRuntimeAction("connect");
  }

  /**
   * TUN preflight: native component + provider
   * path checks, then on-demand elevation (one native prompt, no stored
   * password). Returns false when TUN cannot (or should not) be enabled.
   */
  async function ensureTunPreconditions(): Promise<boolean> {
    const current = await tunStatus();
    if (current.backend !== "process" && !current.nativeComponentReady) {
      pushToast({
        description:
          tunProviderErrorDescription(current, t) ??
          t("status.nativeTunnelMissing"),
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

  async function runTunChange(enabled: boolean) {
    // `modeBusy` also covers a pending connect/disconnect/restart: flipping TUN
    // while the core is still starting persists the flag but cannot restart the
    // not-yet-connected core, leaving the UI claiming TUN over a non-TUN core.
    if (modeBusy || runtimeActionPending() || enabled === tunEnabled) {
      return;
    }

    useRuntimeActionStore.setState({ modePending: true });
    try {
      if (enabled && !(await ensureTunPreconditions())) {
        return;
      }
      await setConnectionMode(enabled ? "vpn" : "systemProxy", null);
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.connectionModeChangeFailed"),
      });
      return;
    } finally {
      await refreshStatus();
      useRuntimeActionStore.setState({ modePending: false });
    }
  }

  function changeTunEnabled(enabled: boolean) {
    void runTunChange(enabled);
  }

  function restart() {
    void runRuntimeAction("restart");
  }

  return {
    activeTunBackend: coreState?.activeTunBackend ?? null,
    nodeEntry,
    busy,
    changeTunEnabled,
    connected,
    tunEnabled,
    sysProxy,
    handlePrimaryAction,
    inProgress,
    mainPid: coreState?.mainPid ?? null,
    modeBusy,
    modePending,
    profiles: profilesQuery.data?.entries ?? [],
    profilesPending: profilesQuery.isPending,
    profilesError: profilesQuery.error,
    restart,
    runningId,
    state,
    switchingId,
    tunProviderSummary,
    tunIssue: tun?.providerPathMismatch
      ? tunProviderPathMismatchDescription(tun, t)
      : tun &&
          (["error", "permissionRequired", "missingComponent"].includes(
            tun.providerState,
          ) ||
            tun.lastProviderError)
        ? tunProviderSummary
        : null,
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
  const description = tunProviderErrorDescription(tun, t);
  if (description) {
    return `${backend}: ${providerState}: ${description}`;
  }

  return `${backend}: ${providerState}`;
}

function tunProviderErrorDescription(tun: TunStatus, t: Translation) {
  if (
    tun.backend === "macosPacketTunnel" &&
    tun.providerState === "missingComponent"
  ) {
    return t("status.macosTunnelMissing");
  }

  return tun.lastProviderError;
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

function tunProviderStateLabel(
  state: TunStatus["providerState"],
  t: Translation,
) {
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
