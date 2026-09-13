import { describe, expect, it } from "vitest";

import type { ProfileListEntry } from "@/ipc/bindings";

import { appendMatcherLine, describeOutbound, nodeOutboundNames } from "./rule-outbound";

describe("rule outbounds", () => {
  it("lists each routable node name once, in list order", () => {
    expect(
      nodeOutboundNames([
        entry("Tokyo"),
        entry(""),
        entry("  "),
        entry("direct"),
        entry("Osaka"),
        entry("Tokyo"),
      ]),
    ).toEqual(["Tokyo", "Osaka"]);
  });

  it("describes built-in tags, a missing outbound, and node targets", () => {
    const nodes = ["Tokyo"];

    expect(describeOutbound(null, nodes)).toEqual({ kind: "proxy" });
    expect(describeOutbound(" ", nodes)).toEqual({ kind: "proxy" });
    expect(describeOutbound("direct", nodes)).toEqual({ kind: "direct" });
    expect(describeOutbound("block", nodes)).toEqual({ kind: "block" });
    expect(describeOutbound("Tokyo", nodes)).toEqual({ kind: "node", name: "Tokyo" });
    expect(describeOutbound("Paris", nodes)).toEqual({ kind: "missing", name: "Paris" });
    // Only the tags themselves are built in, not anything an object inherits.
    expect(describeOutbound("toString", nodes)).toEqual({ kind: "missing", name: "toString" });
  });

  it("reports nothing missing before the node list loads", () => {
    expect(describeOutbound("Paris", null)).toEqual({ kind: "node", name: "Paris" });
  });

  it("names policy groups by id and reports a deleted one", () => {
    const groups = [{ id: "work", name: "Work" }];

    expect(describeOutbound("group:work", [], groups)).toEqual({ kind: "group", name: "Work" });
    expect(describeOutbound("group:gone", [], groups)).toEqual({ kind: "missingGroup", name: "gone" });
    expect(describeOutbound("group:work", [], null)).toEqual({ kind: "group", name: "work" });
    // A node named like a group reference is not offered as a node target.
    expect(nodeOutboundNames([entry("group:work"), entry("Tokyo")])).toEqual(["Tokyo"]);
  });

  it("adds a rule-set line once", () => {
    expect(appendMatcherLine("", "geosite:cn")).toBe("geosite:cn");
    expect(appendMatcherLine("domain:a.test\n", "geosite:cn")).toBe("domain:a.test\ngeosite:cn");
    expect(appendMatcherLine("geosite:cn", "geosite:cn")).toBe("geosite:cn");
  });
});

function entry(remarks: string): ProfileListEntry {
  return { profile: { remarks } } as ProfileListEntry;
}
