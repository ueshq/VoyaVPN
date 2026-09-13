import { describe, expect, it } from "vitest";

import type { TranslationFunction } from "@voya/i18n";

import { PER_APP_SENTINEL, ruleDisplayName, sentinelLabelKey } from "./sentinel-rules";

const t = ((key: string) => `t(${key})`) as TranslationFunction;

describe("sentinel rules", () => {
  it("labels every managed rule and nothing else", () => {
    for (const remarks of [
      "voya:ai-services",
      "voya:block-ads",
      "voya:block-quic",
      "voya:bypass-lan",
      "voya:cn-direct",
      "voya:cn-dns",
      PER_APP_SENTINEL,
    ]) {
      expect(sentinelLabelKey(remarks)).toMatch(/^panes\.routing\.sentinel\./);
    }
    expect(sentinelLabelKey("voya:unknown")).toBeNull();
    expect(sentinelLabelKey("toString")).toBeNull();
    expect(sentinelLabelKey(null)).toBeNull();
  });

  it("names managed rules by label, user rules by remarks, and blank rules as untitled", () => {
    expect(ruleDisplayName({ remarks: "voya:block-ads" }, t)).toBe(
      "t(panes.routing.sentinel.blockAds)",
    );
    expect(ruleDisplayName({ remarks: "Office" }, t)).toBe("Office");
    expect(ruleDisplayName({ remarks: "  " }, t)).toBe("t(panes.routing.untitled)");
    expect(ruleDisplayName({ remarks: null }, t)).toBe("t(panes.routing.untitled)");
  });
});
