import { describe, expect, it } from "vitest";

import { splitList, trimToNull } from "./text";

describe("text", () => {
  it("trims a value and reads blank input as unset", () => {
    expect(trimToNull("  eth0 ")).toBe("eth0");
    expect(trimToNull("   ")).toBeNull();
    expect(trimToNull("")).toBeNull();
    expect(trimToNull(null)).toBeNull();
    expect(trimToNull(undefined)).toBeNull();
  });

  it("splits lines and commas into trimmed entries without blanks", () => {
    expect(splitList(" h2, http/1.1\n\n,grpc ")).toEqual(["h2", "http/1.1", "grpc"]);
    expect(splitList(" , \n")).toEqual([]);
    expect(splitList(null)).toEqual([]);
    expect(splitList(undefined)).toEqual([]);
  });
});
