import { describe, expect, it } from "vitest";

import type { ProfileTransport } from "@/ipc/bindings";

import { profileTransportName } from "./profile-display";

describe("profile display projections", () => {
  it.each([
    [null, "tcp"],
    [{ header: null, host: null, kind: "tcp", path: null }, "tcp"],
    [{ host: null, kind: "websocket", path: null }, "ws"],
    [{ host: null, kind: "httpUpgrade", path: null }, "httpupgrade"],
    [{ host: null, kind: "http2", path: null }, "h2"],
    [{ authority: null, kind: "grpc", mode: null, serviceName: null }, "grpc"],
  ] as Array<[ProfileTransport | null, string]>)("maps %j to %s", (transport, expected) => {
    expect(profileTransportName(transport)).toBe(expected);
  });
});
