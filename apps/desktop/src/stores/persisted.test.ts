import { describe, expect, it } from "vitest";

import { mergeValidated } from "./persisted";

type State = { count: number; label: string };

const merge = mergeValidated<State>((stored) =>
  typeof stored.count === "number" ? { count: stored.count } : {},
);

describe("mergeValidated", () => {
  const current: State = { count: 0, label: "default" };

  it("takes the valid stored fields and keeps the rest", () => {
    expect(merge({ count: 3, label: 7 }, current)).toEqual({ count: 3, label: "default" });
  });

  it("keeps the current state when nothing usable was stored", () => {
    expect(merge({ count: "three" }, current)).toEqual(current);
    expect(merge(null, current)).toBe(current);
    expect(merge(["count"], current)).toBe(current);
  });
});
