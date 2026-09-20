import { createMockBackend } from "@voya/client/mock-backend";
import {
  makeConnection,
  makeProfileEntry,
  makeRouting,
  makeRoutingRule,
  makeSubscription,
  makeSubscriptionMetadata,
} from "@voya/client/mock-seed";
import type { VoyaCommands, VoyaEventName, VoyaEventPayload } from "@voya/contracts";

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
 * The backend this build talks to.
 *
 * There is only one today: the shared in-memory mock. The native module that
 * fronts the Rust host is not built yet, and when it is, this is the single
 * place that chooses between them — nothing downstream of `setVoyaCommands`
 * can tell which one answered.
 */
export function createTransport(): VoyaTransport {
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
