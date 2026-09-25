import type {
  AppErrorEntity,
  ImportProfilesResult,
  InvalidationScope,
  PolicyGroupRuntime,
  ProfileDetails,
  ProfileSummaryEntry,
  ProxyMonitorStatus,
  SubscriptionUpdateResult,
  VoyaCommands,
  VoyaEventName,
  VoyaEventPayload,
} from "@voya/contracts";
import { VOYA_COMMAND_WIRE } from "@voya/contracts/commands";

import { IpcCommandError } from "./errors";
import {
  makeMockSeed,
  makeProfileEntry,
  makeSubscription,
  type MockSeed,
} from "./mock-seed";

/** What the mock keeps between commands; reached as `MockBackend["state"]`. */
type MockState = MockSeed & {
  /** Every command the app made, in order, for a test to assert on. */
  calls: Array<{ command: keyof VoyaCommands; args: readonly unknown[] }>;
  speedtestRunning: boolean;
  logStreaming: boolean;
  proxyMonitorRunning: boolean;
};

export type MockBackend = {
  /** The whole `VoyaCommands` surface; unimplemented commands reject as `unsupported`. */
  commands: VoyaCommands;
  /** Subscribes to one of the three backend channels. Returns an unsubscribe. */
  on: <Name extends VoyaEventName>(
    name: Name,
    listener: (payload: VoyaEventPayload<Name>) => void,
  ) => () => void;
  /** Publishes on a channel, the way the backend would. */
  emit: <Name extends VoyaEventName>(name: Name, payload: VoyaEventPayload<Name>) => void;
  state: MockState;
};

/**
 * An in-memory backend behind the `VoyaCommands` contract.
 *
 * It exists because every frontend now reaches the backend through one seam:
 * registering this instead of a real transport gives a React Native test, the
 * mobile app's development build and (in time) the renderer smoke the same
 * behaviour, typed against the same generated contract.
 *
 * It models what the real backend guarantees rather than what a screen happens
 * to need — in particular, a committed mutation publishes an invalidation
 * event, because the frontend no longer invalidates for itself.
 */
export function createMockBackend(seed: Partial<MockSeed> = {}): MockBackend {
  const state: MockState = {
    ...makeMockSeed(seed),
    calls: [],
    logStreaming: false,
    proxyMonitorRunning: false,
    speedtestRunning: false,
  };

  const listeners = new Map<VoyaEventName, Set<(payload: never) => void>>();

  function on<Name extends VoyaEventName>(
    name: Name,
    listener: (payload: VoyaEventPayload<Name>) => void,
  ) {
    const forChannel = listeners.get(name) ?? new Set();
    listeners.set(name, forChannel);
    forChannel.add(listener as (payload: never) => void);

    return () => {
      forChannel.delete(listener as (payload: never) => void);
    };
  }

  function emit<Name extends VoyaEventName>(name: Name, payload: VoyaEventPayload<Name>) {
    for (const listener of listeners.get(name) ?? []) {
      (listener as (value: VoyaEventPayload<Name>) => void)(payload);
    }
  }

  /** Announces a committed mutation, exactly as `voya_app::invalidation` does. */
  function invalidate(reason: keyof VoyaCommands, ...kinds: InvalidationScope["kind"][]) {
    emit("invalidateEvent", {
      keys: kinds.map((kind) => ({ reason, scope: { kind } as InvalidationScope })),
    });
  }

  function record<T>(command: keyof VoyaCommands, args: readonly unknown[], result: T): T {
    state.calls.push({ args, command });
    return result;
  }

  function setCoreState(next: MockState["runtime"]) {
    state.runtime = next;
    emit("transientStreamEvent", { kind: "coreState", payload: next });
    return next;
  }

  /**
   * Whether a saved setting is still waiting for the core to pick it up.
   *
   * Nothing here defers: the mock applies a save at once, so there is never a
   * pending action to report.
   */
  function settingsApplyStatus() {
    return { action: "none" as const, connected: state.runtime.state === "connected" };
  }

  /**
   * A phone captures traffic only through its tunnel provider, and the desktop
   * mock follows the same rule as macOS: the tunnel is the mode, and process
   * rules are the one thing a packet tunnel cannot do.
   */
  function connectionMode() {
    return {
      mode: "vpn" as const,
      processRulesEffective: false,
      processRulesSupported: false,
      systemProxyAvailable: false,
      vpnAvailable: true,
    };
  }

  const implemented: Partial<VoyaCommands> = {
    checkConnectionIp: () =>
      resolve(
        record("checkConnectionIp", [], {
          countryCode: "JP",
          ip: "203.0.113.42",
        }),
      ),

    connectActiveProfile: () => {
      const activeProfileId = state.profiles.find((entry) => entry.isActive)?.profile.id ?? null;
      return resolve(
        record(
          "connectActiveProfile",
          [],
          setCoreState({
            activeProfileId,
            activeTunBackend: null,
            connectedDurationMs: 0,
            mainPid: 4242,
            prePid: null,
            state: "connected",
          }),
        ),
      );
    },

    disconnectCore: () =>
      resolve(
        record(
          "disconnectCore",
          [],
          setCoreState({
            activeProfileId: null,
            activeTunBackend: null,
            connectedDurationMs: null,
            mainPid: null,
            prePid: null,
            state: "disconnected",
          }),
        ),
      ),

    previewImportProfiles: (text) => {
      const links = text.split(/\s+/).filter((line) => line.includes("://"));
      return resolve(record("previewImportProfiles", [text], {
        nodes: links.filter((link) => !/^https?:/.test(link)).map((link) => ({ name: profileNameFromShareLink(link) ?? "Imported", protocol: link.split(":")[0], address: "example.test" })),
        subscriptionUrls: links.filter((link) => /^https?:/.test(link)), failed: 0, lineIssues: [],
      }));
    },
    importProfilesFromText: (text, subscriptionId) => {
      const links = text
        .split(/\s+/)
        .map((line) => line.trim())
        .filter((line) => line.includes("://"));
      const subscriptionUrls = links.filter((link) => /^https?:\/\//.test(link));
      const shareLinks = links.filter((link) => !/^https?:\/\//.test(link));

      const importedProfileIds = shareLinks.map((link, offset) => {
        const entry = makeProfileEntry(state.profiles.length + offset, {
          remarks: profileNameFromShareLink(link) ?? `Imported ${state.profiles.length + offset}`,
          subscriptionId,
        }, false);
        state.profiles.push(entry);
        return entry.profile.id;
      });

      const addedSubscriptionIds = subscriptionUrls.map((url) => {
        const subscription = makeSubscription(state.subscriptions.length, { url });
        state.subscriptions.push(subscription);
        return subscription.id;
      });

      const result: ImportProfilesResult = {
        addedSubscriptionIds,
        deduped: 0,
        discardedNodeOverrides: 0,
        failed: 0,
        filtered: 0,
        imported: importedProfileIds.length,
        importedProfileIds,
        lineIssues: [],
        parsed: links.length,
        removedDuplicates: 0,
        removedExisting: 0,
        skipped: 0,
        subscriptionId,
        updated: 0,
        updatedProfileIds: [],
      };

      invalidate("importProfilesFromText", "profiles", "subscriptions", "subscriptionMetadata");
      return resolve(record("importProfilesFromText", [text, subscriptionId], result));
    },

    /** Share links for the named nodes, in the order they were asked for. */
    exportProfileShareLinks: (indexIds) => {
      const links = indexIds.flatMap((indexId) => {
        const entry = state.profiles.find((item) => item.profile.id === indexId);
        return entry
          ? [`vless://token@${entry.profile.address}:${entry.profile.port}#${entry.profile.remarks}`]
          : [];
      });

      return resolve(
        record("exportProfileShareLinks", [indexIds], {
          count: links.length,
          text: links.join("\n"),
        }),
      );
    },

    deleteProfiles: (indexIds) => {
      const before = state.profiles.length;
      state.profiles = state.profiles.filter((item) => !indexIds.includes(item.profile.id));
      invalidate("deleteProfiles", "profiles");
      return resolve(record("deleteProfiles", [indexIds], before - state.profiles.length));
    },

    /**
     * A QR code shaped like one without being one.
     *
     * The real command renders through `rqrr`; nothing on the frontend reads
     * the modules back, so what matters here is that it is SVG and that it
     * carries the content it was given.
     */
    generateQrCode: (content) =>
      resolve(
        record("generateQrCode", [content], {
          mimeType: "image/svg+xml",
          svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><title>${content.length} bytes</title><rect width="1" height="1" fill="#000"/></svg>`,
        }),
      ),

    listPolicyGroups: () =>
      resolve(record("listPolicyGroups", [], { entries: state.policyGroups })),

    setActivePolicyGroup: (id) => {
      const entry = state.policyGroups.find((item) => item.group.id === id);
      if (!entry) return reject("profile", id, `no policy group with id ${id}`);
      // A group replaces the node used on its own, the way the backend does.
      state.policyGroups = state.policyGroups.map((item) => ({
        ...item,
        isActive: item.group.id === id,
      }));
      state.profiles = state.profiles.map((item) => ({ ...item, isActive: false }));
      invalidate("setActivePolicyGroup", "policyGroups", "profiles");
      return resolve(record("setActivePolicyGroup", [id], entry.group));
    },


    listRoutings: () => resolve(record("listRoutings", [], state.routings)),

    getDefaultDnsSettings: () => resolve(record("getDefaultDnsSettings", [], { ...makeMockSeed().settings.dns })),
    loadDnsSettings: () => resolve(record("loadDnsSettings", [], state.settings.dns)),

    saveDnsSettings: (settings) => {
      state.settings = { ...state.settings, dns: settings };
      invalidate("saveDnsSettings", "dns", "appSettings");
      return resolve(record("saveDnsSettings", [settings], settings));
    },

    saveAppSettings: (settings) => {
      state.settings = settings;
      invalidate("saveAppSettings", "appSettings", "uiPreferences", "dns", "connectionMode");
      return resolve(record("saveAppSettings", [settings], settings));
    },

    getSettingsApplyStatus: () =>
      resolve(record("getSettingsApplyStatus", [], settingsApplyStatus())),

    applyPendingSettings: () =>
      resolve(record("applyPendingSettings", [], settingsApplyStatus())),

    connectionModeStatus: () =>
      resolve(record("connectionModeStatus", [], connectionMode())),

    saveRouting: (item) => {
      // `Routing_Deserialize` in, `Routing_Serialize` out: the active flag is
      // the backend's answer, never the caller's claim.
      const saved = {
        ...item,
        id: item.id || `routing-${state.routings.length}`,
        isActive: state.routings.some(
          (routing) => routing.isActive && routing.id === item.id,
        ),
      };
      state.routings = upsert(state.routings, saved);
      invalidate("saveRouting", "routings", "appSettings");
      return resolve(record("saveRouting", [item], saved));
    },

    setActiveRouting: (id) => {
      const target = state.routings.find((routing) => routing.id === id);
      if (!target) return reject("routing", id, `no routing with id ${id}`);
      state.routings = state.routings.map((routing) => ({
        ...routing,
        isActive: routing.id === id,
      }));
      invalidate("setActiveRouting", "routings", "appSettings");
      return resolve(record("setActiveRouting", [id], { ...target, isActive: true }));
    },

    deleteRoutings: (ids) => {
      const before = state.routings.length;
      state.routings = state.routings.filter((routing) => !ids.includes(routing.id));
      invalidate("deleteRoutings", "routings", "appSettings");
      return resolve(record("deleteRoutings", [ids], before - state.routings.length));
    },

    saveRoutingRule: (routingId, rule) => {
      const routing = state.routings.find((item) => item.id === routingId);
      if (!routing) return reject("routing", routingId, `no routing with id ${routingId}`);
      const saved = { ...rule, id: rule.id || `rule-${routing.rules.length}` };
      const updated = {
        ...routing,
        rules: routing.rules.some((item) => item.id === saved.id)
          ? routing.rules.map((item) => (item.id === saved.id ? saved : item))
          : [...routing.rules, saved],
      };
      state.routings = upsert(state.routings, updated);
      invalidate("saveRoutingRule", "routings");
      return resolve(record("saveRoutingRule", [routingId, rule], updated));
    },

    deleteRoutingRules: (routingId, ruleIds) => {
      const routing = state.routings.find((item) => item.id === routingId);
      if (!routing) return reject("routing", routingId, `no routing with id ${routingId}`);
      const updated = {
        ...routing,
        rules: routing.rules.filter((rule) => !ruleIds.includes(rule.id)),
      };
      state.routings = upsert(state.routings, updated);
      invalidate("deleteRoutingRules", "routings");
      return resolve(record("deleteRoutingRules", [routingId, ruleIds], updated));
    },

    proxyListConnections: () =>
      resolve(record("proxyListConnections", [], state.connections)),

    // A null id is the command's own "all of them", the same as the backend.
    proxyCloseConnection: (connectionId) => {
      state.connections = {
        ...state.connections,
        connections:
          connectionId === null
            ? []
            : state.connections.connections.filter((item) => item.id !== connectionId),
      };
      invalidate("proxyCloseConnection", "proxyConnections");
      return resolve(record("proxyCloseConnection", [connectionId], state.connections));
    },

    proxyStartMonitor: () => {
      state.proxyMonitorRunning = true;
      return resolve(record("proxyStartMonitor", [], monitorStatus(true)));
    },

    proxyStopMonitor: () => {
      state.proxyMonitorRunning = false;
      return resolve(record("proxyStopMonitor", [], monitorStatus(false)));
    },

    proxySetTrafficMode: (mode) => {
      state.settings = { ...state.settings, proxy: { ...state.settings.proxy, trafficMode: mode } };
      invalidate("proxySetTrafficMode", "proxyConnections", "appSettings");
      return resolve(record("proxySetTrafficMode", [mode], { mode }));
    },

    listProfileSummaries: () =>
      resolve(
        record("listProfileSummaries", [], {
          entries: state.profiles,
          undecodableProfiles: 0,
        }),
      ),

    listSubscriptionMetadata: () =>
      resolve(record("listSubscriptionMetadata", [], state.subscriptionMetadata)),

    listSubscriptions: () => resolve(record("listSubscriptions", [], state.subscriptions)),
    saveSubscription: (source) => {
      const existing = state.subscriptions.findIndex((item) => item.id === source.id);
      const saved = { ...source };
      if (existing >= 0) state.subscriptions[existing] = saved;
      else state.subscriptions.push(saved);
      invalidate("saveSubscription", "subscriptions");
      return resolve(record("saveSubscription", [source], saved));
    },
    deleteSubscriptions: (ids) => {
      const removed = state.subscriptions.filter((source) => ids.includes(source.id)).length;
      state.subscriptions = state.subscriptions.filter((source) => !ids.includes(source.id));
      state.profiles = state.profiles.filter((entry) => !ids.includes(entry.profile.subscriptionId ?? ""));
      invalidate("deleteSubscriptions", "subscriptions", "profiles");
      return resolve(record("deleteSubscriptions", [ids], removed));
    },

    loadAppSettings: () => resolve(record("loadAppSettings", [], state.settings)),

    loadUiPreferences: () =>
      resolve(record("loadUiPreferences", [], state.settings.appearance)),

    policyGroupRuntime: () => {
      const active = state.policyGroups.find((entry) => entry.isActive);
      const runtime: PolicyGroupRuntime | null = active
        ? {
            groupId: active.group.id,
            members: active.members.map((member) => ({
              delayMs: 0,
              profileId: member.profileId,
              remarks: member.remarks,
            })),
            nowProfileId: active.group.selectedProfileId ?? active.members[0]?.profileId ?? null,
          }
        : null;
      return resolve(record("policyGroupRuntime", [], runtime));
    },

    restartCore: () => {
      const activeProfileId = state.profiles.find((entry) => entry.isActive)?.profile.id ?? null;
      return resolve(
        record(
          "restartCore",
          [],
          setCoreState({
            activeProfileId,
            activeTunBackend: null,
            connectedDurationMs: 0,
            mainPid: 4243,
            prePid: null,
            state: "connected",
          }),
        ),
      );
    },

    runtimeStatus: () => resolve(record("runtimeStatus", [], state.runtime)),

    setActiveProfile: (indexId) => {
      const selected = state.profiles.find((entry) => entry.profile.id === indexId);
      if (!selected) {
        return reject("profile", indexId, `no node with id ${indexId}`);
      }
      state.profiles = state.profiles.map((entry) => ({
        ...entry,
        isActive: entry.profile.id === indexId,
      }));
      // A node in use replaces the group in use, so both listings move.
      state.policyGroups = state.policyGroups.map((entry) => ({ ...entry, isActive: false }));
      invalidate("setActiveProfile", "profiles", "policyGroups", "appSettings");
      return resolve(record("setActiveProfile", [indexId], profileDetails(selected)));
    },

    setLogStreaming: (enabled) => {
      state.logStreaming = enabled;
      return resolve(record("setLogStreaming", [enabled], null));
    },

    speedtestStatus: () =>
      resolve(record("speedtestStatus", [], { running: state.speedtestRunning })),

    /**
     * Measures every selected node at once, the way a very fast backend would.
     *
     * The results stream on the transient channel *and* come back in the
     * answer, because that is what the real backend does; a screen that only
     * reads one of the two would look right here and be wrong on a device.
     */
    runSpeedtest: (request) => {
      const profileIds = request.target.profileIds;
      state.speedtestRunning = true;
      const results = profileIds.map((indexId, index) => ({
        countryCode: null,
        delay: 40 + index * 7,
        detail: null,
        indexId,
        ipInfo: null,
        outcome: "completed" as const,
      }));
      state.profiles = state.profiles.map((entry) => {
        const result = results.find((item) => item.indexId === entry.profile.id);
        return result
          ? { ...entry, metrics: { ...entry.metrics, delayMs: result.delay, outcome: "completed" } }
          : entry;
      });
      emit("transientStreamEvent", { kind: "speedtestResults", payload: results });
      state.speedtestRunning = false;
      invalidate("runSpeedtest", "profiles");

      return resolve(
        record("runSpeedtest", [request], {
          cancelled: false,
          completedCount: results.length,
          results,
          selectedCount: profileIds.length,
        }),
      );
    },

    updateSrsAssets: () =>
      resolve(
        record("updateSrsAssets", [], [{ bytes: 131_072, name: "geosite-cn.srs", usedProxy: false }]),
      ),

    cancelSpeedtest: () => {
      state.speedtestRunning = false;
      return resolve(record("cancelSpeedtest", [], { running: false }));
    },

    systemProxyStatus: () => resolve(record("systemProxyStatus", [], state.sysProxy)),

    tunStatus: () => resolve(record("tunStatus", [], state.tun)),

    updateSubscriptions: (subscriptionId, preferProxy, proxyUrl) => {
      const targets = subscriptionId === null
        ? state.subscriptions.map((item) => item.id)
        : [subscriptionId];
      const imported = targets.flatMap((subscriptionId) => {
        if (!state.subscriptions.some((item) => item.id === subscriptionId)) return [];
        const entry = makeProfileEntry(state.profiles.length, { subscriptionId }, false);
        state.profiles.push(entry);
        return [entry];
      });

      const result: SubscriptionUpdateResult = {
        imported: imported.length,
        outcomes: targets.map((id) => ({ subscriptionId: id, status: "success", reason: "updated", imported: 1, removedExisting: 0, diagnostic: null })),
        messages: [],
        removedExisting: 0,
        skipped: targets.length - imported.length,
        updated: 0,
      };

      invalidate("updateSubscriptions", "profiles", "subscriptions", "subscriptionMetadata");
      return resolve(
        record("updateSubscriptions", [subscriptionId, preferProxy, proxyUrl], result),
      );
    },
  };

  return {
    commands: { ...unsupportedCommands(), ...implemented },
    emit,
    on,
    state,
  };
}

/**
 * Answers a command with a copy, the way a transport does.
 *
 * Every real transport serializes: Tauri IPC and the native module both hand
 * the frontend a value built from JSON, so no two responses ever share a
 * reference with each other or with backend state. Returning the live objects
 * instead made a mutation *invisible* — React Query's structural sharing sees
 * the array it already holds, keeps the old reference, and every `useMemo`
 * keyed on it skips the rebuild. That is a difference a screen can see, so the
 * mock has to cross the same boundary.
 */
/** The monitor reports itself the way the real one does: never stale, never failed. */
function monitorStatus(running: boolean): ProxyMonitorStatus {
  return { message: null, running, stale: false, state: running ? "running" : "stopped" };
}

/** Replaces an item by id, or appends it. */
function upsert<T extends { id: string }>(items: T[], item: T): T[] {
  return items.some((existing) => existing.id === item.id)
    ? items.map((existing) => (existing.id === item.id ? item : existing))
    : [...items, item];
}

function resolve<T>(value: T): Promise<T> {
  return Promise.resolve(value === undefined ? value : (JSON.parse(JSON.stringify(value)) as T));
}

function reject(entity: AppErrorEntity, id: string, message: string): Promise<never> {
  return Promise.reject(
    new IpcCommandError({
      kind: { entity, id, type: "notFound" },
      message,
      subsystem: entity === "profile" ? "profile" : "app",
    }),
  );
}

/** A summary entry as `get_profile` would return it: enough for what follows a switch. */
function profileDetails(entry: ProfileSummaryEntry): ProfileDetails {
  return {
    isActive: true,
    metrics: entry.metrics,
    profile: {
      displayLog: false,
      id: entry.profile.id,
      protocol: {
        cipher: "auto",
        kind: "vmess",
        server: { address: entry.profile.address, port: entry.profile.port },
        uuid: `uuid-${entry.profile.id}`,
      },
      remarks: entry.profile.remarks,
      subscriptionId: entry.profile.subscriptionId,
      tls: null,
      transport: null,
    },
    traffic: {
      date: null,
      todayDownload: null,
      todayUpload: null,
      totalDownload: null,
      totalUpload: null,
    },
  };
}

/**
 * Every command, rejecting as `unsupported`.
 *
 * Built from the generated wire table rather than written out, so a command
 * added in Rust is automatically present here — and reaches a screen as the
 * typed error a platform without that capability would send, rather than as
 * `undefined is not a function`.
 */
function unsupportedCommands(): VoyaCommands {
  const entries = Object.keys(VOYA_COMMAND_WIRE).map((name) => [
    name,
    () =>
      Promise.reject(
        new IpcCommandError({
          kind: { type: "unsupported" },
          message: `${name} is not implemented by the mock backend`,
          subsystem: "app",
        }),
      ),
  ]);

  // Sound by construction: `VOYA_COMMAND_WIRE` is keyed by `keyof VoyaCommands`
  // and every value rejects, which satisfies any declared return type.
  return Object.fromEntries(entries) as VoyaCommands;
}

/** The name a `vless://…#Tokyo` style share link carries, if any. */
function profileNameFromShareLink(link: string) {
  const fragment = link.split("#")[1];
  if (!fragment) return null;
  try {
    return decodeURIComponent(fragment) || null;
  } catch {
    // A malformed escape is the link's problem, not a reason to drop the node.
    return fragment;
  }
}
