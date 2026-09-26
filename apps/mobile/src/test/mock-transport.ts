import { createMockBackend, type MockBackend } from "@voya/client/mock-backend";

import { voyaTransport } from "~/ipc/platform";
import {
  makeConnection,
  makeProfileEntry,
  makeRouting,
  makeRoutingRule,
  makeSubscription,
  makeSubscriptionMetadata,
} from "@voya/client/mock-seed";

/** The in-memory backend screen tests register in place of the native module. */
export function mockTransport() {
  return createMockBackend({
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
}

/** The registered backend, as the mock a test registered. */
export function mockBackend() {
  return voyaTransport() as MockBackend;
}
