import { createMockBackend } from "@voya/client/mock-backend";
import { NativeEventEmitter, TurboModuleRegistry } from "react-native";
import {
  makeConnection,
  makeProfileEntry,
  makeRouting,
  makeRoutingRule,
  makeSubscription,
  makeSubscriptionMetadata,
} from "@voya/client/mock-seed";
import type { VoyaCommands, VoyaEventName, VoyaEventPayload } from "@voya/contracts";

import {
  createNativeTransport,
  type VoyaNativeEvents,
  type VoyaNativeModule,
} from "./native-transport";

/**
 * What this app needs of a backend: the command surface and the three channels.
 *
 * The same shape the shared mock backend has, so registering either one is the
 * only difference between a development build and a device build.
 */
export type VoyaTransport = {
  commands: VoyaCommands;
  on: <Name extends VoyaEventName>(
    name: Name,
    listener: (payload: VoyaEventPayload<Name>) => void,
  ) => () => void;
};

/**
 * The name the native module registers itself under, on both platforms.
 *
 * `TurboModuleRegistry.get` rather than `getEnforcing`: a development build
 * has no native module and must fall back to the mock rather than throw at
 * import time.
 */
const NATIVE_MODULE_NAME = "VoyaNative";

/**
 * The backend this build talks to.
 *
 * The native module when this build has one, and the shared in-memory mock
 * when it does not. This is the only place that chooses — nothing downstream
 * of `setVoyaCommands` can tell which one answered, which is what makes a
 * development build a rehearsal for a device build rather than a separate app.
 */
export function createTransport(): VoyaTransport {
  const native = TurboModuleRegistry.get<VoyaNativeModule>(NATIVE_MODULE_NAME);

  return native ? createNativeTransport(native, nativeEvents(native)) : createMockTransport();
}

/**
 * The host publishes all three channels as one native event with the channel
 * in its payload, so there is one listener rather than three registrations to
 * keep in step.
 */
function nativeEvents(native: VoyaNativeModule): VoyaNativeEvents {
  const emitter = new NativeEventEmitter(native);

  return {
    addListener: (name, listener) =>
      emitter.addListener(name, (payload) =>
        listener(payload as { channel: string; payloadJson: string }),
      ),
  };
}

function createMockTransport(): VoyaTransport {
  return createMockBackend({
    // Live connections only show while connected, which is exactly when this
    // seed is on screen: connect in a development build and the Activity list
    // has something in it.
    connections: {
      connections: [
        makeConnection(0, { host: "example.test", process: "Safari" }),
        makeConnection(1, { chains: ["direct"], host: "cdn.example.test", process: "Music" }),
      ],
      downloadTotal: 4096,
      uploadTotal: 2048,
    },
    profiles: [
      makeProfileEntry(0, { remarks: "🇯🇵 Tokyo" }),
      makeProfileEntry(1, { remarks: "🇸🇬 Singapore" }),
      makeProfileEntry(2, { remarks: "🇺🇸 Los Angeles", subscriptionId: "subscription-0" }, false),
    ],
    routings: [
      makeRouting(0, {
        rules: [
          makeRoutingRule(0, { domain: ["example.test"], remarks: "Work" }),
          makeRoutingRule(1, { outbound: "direct", process: ["Music"], remarks: "Music direct" }),
        ],
      }),
    ],
    subscriptionMetadata: [makeSubscriptionMetadata("subscription-0")],
    subscriptions: [makeSubscription(0, { remarks: "Example provider" })],
  });
}
