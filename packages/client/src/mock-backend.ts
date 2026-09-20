import type {
  AppErrorEntity,
  ImportProfilesResult,
  InvalidationScope,
  PolicyGroupRuntime,
  ProfileDetails,
  ProfileSummaryEntry,
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

    listPolicyGroups: () =>
      resolve(record("listPolicyGroups", [], { entries: state.policyGroups })),

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

function resolve<T>(value: T): Promise<T> {
  return Promise.resolve(value);
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
