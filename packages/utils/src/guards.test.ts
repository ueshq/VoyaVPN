import { describe, expect, it } from "vitest";

import { isRecord } from "./guards";

describe("isRecord", () => {
  it("accepts objects and rejects null, arrays and primitives", () => {
    expect(isRecord({})).toBe(true);
    expect(isRecord({ theme: "dark" })).toBe(true);
    expect(isRecord(null)).toBe(false);
    expect(isRecord([])).toBe(false);
    expect(isRecord("dark")).toBe(false);
    expect(isRecord(undefined)).toBe(false);
  });
});
