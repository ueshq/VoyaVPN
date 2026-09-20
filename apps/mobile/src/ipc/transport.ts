import { createMockBackend } from "@voya/client/mock-backend";
import { makeProfileEntry, makeSubscription, makeSubscriptionMetadata } from "@voya/client/mock-seed";
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
    profiles: [
      makeProfileEntry(0, { remarks: "🇯🇵 Tokyo" }),
      makeProfileEntry(1, { remarks: "🇸🇬 Singapore" }),
      makeProfileEntry(2, { remarks: "🇺🇸 Los Angeles", subscriptionId: "subscription-0" }, false),
    ],
    subscriptionMetadata: [makeSubscriptionMetadata("subscription-0")],
    subscriptions: [makeSubscription(0, { remarks: "Example provider" })],
  });
}
