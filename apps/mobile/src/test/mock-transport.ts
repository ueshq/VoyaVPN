import { createMockBackend, type MockBackend } from "@voya/client/mock-backend";

import { voyaTransport } from "~/ipc/platform";
import type { VoyaTransport } from "~/ipc/transport";
import {
  makeConnection,
  makeProfileEntry,
  makeRouting,
  makeRoutingRule,
  makeSubscription,
  makeSubscriptionMetadata,
} from "@voya/client/mock-seed";

/**
 * The in-memory backend screen tests register in place of the native module.
 * Its reset succeeds and changes nothing; a test that needs it to mean
 * something replaces it, as it would a command.
 */
export function mockTransport(): MockBackend & Pick<VoyaTransport, "resetApplicationData"> {
  const backend = createMockBackend({
    // Live connections only show while connected, so a test that connects
    // has an Activity list to read.
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

  return Object.assign(backend, { resetApplicationData: async () => {} });
}

/** The registered backend, as the mock a test registered. */
export function mockBackend() {
  return voyaTransport() as ReturnType<typeof mockTransport>;
}
