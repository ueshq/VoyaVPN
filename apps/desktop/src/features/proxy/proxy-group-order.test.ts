import { describe, expect, it } from "vitest";

import type { ProxyGroup } from "@/ipc/bindings";
import { isAutoGroup, orderProxyGroups } from "./proxy-group-order";

function group(name: string, proxyType: string): ProxyGroup {
  return { name, nodes: [], now: null, proxyType };
}

describe("orderProxyGroups", () => {
  it("pins urltest and fallback groups first, keeping order stable otherwise", () => {
    const ordered = orderProxyGroups([
      group("Manual", "Selector"),
      group("Auto", "URLTest"),
      group("Backup", "Fallback"),
      group("Regions", "selector"),
    ]);

    expect(ordered.map((item) => item.name)).toEqual(["Auto", "Backup", "Manual", "Regions"]);
  });

  it("leaves a list without auto groups untouched", () => {
    const groups = [group("A", "selector"), group("B", "selector")];
    expect(orderProxyGroups(groups)).toEqual(groups);
  });

  it("classifies auto groups case-insensitively", () => {
    expect(isAutoGroup(group("Auto", "URLTest"))).toBe(true);
    expect(isAutoGroup(group("Auto", "urltest"))).toBe(true);
    expect(isAutoGroup(group("Sel", "selector"))).toBe(false);
  });
});
