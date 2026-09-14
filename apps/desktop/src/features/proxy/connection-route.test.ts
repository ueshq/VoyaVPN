import { describe, expect, it } from "vitest";

import { connectionRoute } from "./connection-route";

describe("connectionRoute", () => {
  it.each([
    [["PROXY", "Tokyo"], { kind: "proxy", node: "Tokyo" }],
    [["Travel · Auto", "Singapore"], { kind: "proxy", node: "Singapore" }],
    [["proxy"], { kind: "proxy", node: null }],
    [["DIRECT"], { kind: "direct" }],
    [["REJECT"], { kind: "block" }],
    [["block"], { kind: "block" }],
    [[" "], { kind: "unknown" }],
    [[], { kind: "unknown" }],
  ] as const)("reads the chain %j as %j", (chains, expected) => {
    expect(connectionRoute({ chains: [...chains] })).toEqual(expected);
  });
});
