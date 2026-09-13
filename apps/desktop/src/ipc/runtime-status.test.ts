import { beforeEach, describe, expect, it, vi } from "vitest";
import { i18next } from "@voya/i18n";
import { useToastStore } from "@/stores/toast-store";
import type { RuntimeStatusResponse, SystemProxyStatusResponse, TunStatus } from "./bindings";
import { useRuntimeEventStore } from "./runtime-event-store";
import { refreshRuntimeStatus, refreshRuntimeStatusAndReport } from "./runtime-status";

const commands = vi.hoisted(() => ({ runtimeStatus: vi.fn(), systemProxyStatus: vi.fn(), tunStatus: vi.fn() }));
vi.mock("@/ipc/commands", () => commands);

const core: RuntimeStatusResponse = {
  state: "connected", activeTunBackend: null, activeProfileId: "node", mainPid: 1, prePid: null, connectedDurationMs: null, runningCoreType: "singBox",
};
const proxy: SystemProxyStatusResponse = {
  management: "automatic",
  requestedMode: "forcedChange", effectiveMode: "unchanged", proxy: "127.0.0.1:10808",
  exceptions: "",
};
const tun: TunStatus = {
  backend: "macosPacketTunnel", enabled: false, allowEnableTun: true, nativeComponentReady: true,
  elevationGranted: false, requiresElevation: false, needsServiceInstall: false, needsVpnPermission: false,
  providerState: "stopped", lastProviderError: null, expectedProviderPath: null, resolvedProviderPath: null,
  providerPathMismatch: false, restoreOnDisconnect: false,
  preflight: { state: "ready", platform: "macos", notes: [], routeRestoreNote: "", windowsCleanupDevices: [] },
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

describe("runtime status reconciliation", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    useRuntimeEventStore.setState({ coreState: null, sysProxy: null, tun: null });
    useToastStore.setState({ toasts: [] });
    commands.runtimeStatus.mockResolvedValue(core);
    commands.systemProxyStatus.mockResolvedValue(proxy);
    commands.tunStatus.mockResolvedValue(tun);
  });

  it("starts independent reads together and commits each without waiting for the slowest", async () => {
    const slow = deferred<RuntimeStatusResponse>();
    commands.runtimeStatus.mockReturnValue(slow.promise);
    const refresh = refreshRuntimeStatus();
    expect(commands.runtimeStatus).toHaveBeenCalledOnce();
    expect(commands.systemProxyStatus).toHaveBeenCalledOnce();
    expect(commands.tunStatus).toHaveBeenCalledOnce();
    await Promise.resolve();
    expect(useRuntimeEventStore.getState().sysProxy).toEqual(proxy);
    expect(useRuntimeEventStore.getState().tun).toEqual(tun);
    expect(useRuntimeEventStore.getState().coreState).toBeNull();
    slow.resolve(core);
    expect(await refresh).toEqual([]);
  });

  it("does not overwrite newer events even when the same payload is published again", async () => {
    const oldCore = deferred<RuntimeStatusResponse>();
    const oldProxy = deferred<SystemProxyStatusResponse>();
    const oldTun = deferred<TunStatus>();
    commands.runtimeStatus.mockReturnValue(oldCore.promise);
    commands.systemProxyStatus.mockReturnValue(oldProxy.promise);
    commands.tunStatus.mockReturnValue(oldTun.promise);
    const refresh = refreshRuntimeStatus();
    const latest = { ...proxy, proxy: null };
    const store = useRuntimeEventStore.getState();
    store.pushTransientEvent({ kind: "coreState", payload: core });
    store.pushTransientEvent({ kind: "sysProxyChanged", payload: latest });
    store.pushTransientEvent({ kind: "tunChanged", payload: tun });
    oldCore.resolve({ ...core, state: "disconnected" });
    oldProxy.resolve(proxy);
    oldTun.resolve({ ...tun, enabled: true });
    await refresh;
    expect(useRuntimeEventStore.getState()).toMatchObject({ coreState: core, sysProxy: latest, tun });
  });

  it("lets the latest request win and ignores an unmounted seed", async () => {
    const old = deferred<RuntimeStatusResponse>();
    commands.runtimeStatus.mockReturnValueOnce(old.promise);
    const first = refreshRuntimeStatus(["coreState"]);
    await refreshRuntimeStatus(["coreState"]);
    old.resolve({ ...core, mainPid: 99 });
    await first;
    expect(useRuntimeEventStore.getState().coreState).toEqual(core);
    await refreshRuntimeStatus(undefined, () => false);
    expect(useRuntimeEventStore.getState().sysProxy).toBeNull();
  });

  it("returns only current failures while other channels still settle", async () => {
    const error = new Error("cannot inspect proxy");
    commands.systemProxyStatus.mockRejectedValue(error);
    expect(await refreshRuntimeStatus()).toEqual([{ channel: "sysProxy", error }]);
    expect(useRuntimeEventStore.getState().coreState).toEqual(core);
    expect(useRuntimeEventStore.getState().tun).toEqual(tun);
    expect(await refreshRuntimeStatus(["sysProxy"], () => false)).toEqual([]);
  });

  it("reports each failed channel while retaining successful status reads", async () => {
    commands.runtimeStatus.mockRejectedValue(new Error("core read failed"));
    commands.systemProxyStatus.mockRejectedValue(new Error("proxy read failed"));
    commands.tunStatus.mockRejectedValueOnce(new Error("tun read failed"));
    await refreshRuntimeStatusAndReport(i18next.t);
    expect(useToastStore.getState().toasts).toMatchObject([
      { title: i18next.t("status.runtimeStatusFailed"), description: "core read failed", severity: "error" },
      { title: i18next.t("status.sysProxyStatusFailed"), description: "proxy read failed", severity: "error" },
      { title: i18next.t("status.tunStatusFailed"), description: "tun read failed", severity: "error" },
    ]);
    await refreshRuntimeStatusAndReport(i18next.t, ["tun"]);
    expect(useRuntimeEventStore.getState().tun).toEqual(tun);
    expect(useToastStore.getState().toasts).toHaveLength(3);
  });

  it("does not report settled failures after the seed unmounts during another channel read", async () => {
    let mounted = true;
    const slow = deferred<TunStatus>();
    commands.runtimeStatus.mockRejectedValue(new Error("core read failed"));
    commands.tunStatus.mockReturnValueOnce(slow.promise);
    const refresh = refreshRuntimeStatusAndReport(i18next.t, undefined, () => mounted);
    await Promise.resolve();
    mounted = false;
    slow.resolve(tun);
    await refresh;
    expect(useToastStore.getState().toasts).toEqual([]);
    expect(useRuntimeEventStore.getState().tun).toBeNull();
  });
});
