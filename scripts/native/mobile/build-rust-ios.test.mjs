import assert from "node:assert/strict";
import { describe, it } from "vitest";

import { IOS_TARGETS, missingTargets, selectIosTargets } from "./build-rust-ios.mjs";

describe("iOS Rust slices", () => {
  it("builds both slices unless a lane names the one it runs on", () => {
    assert.deepEqual(selectIosTargets([]), IOS_TARGETS);
    assert.deepEqual(
      selectIosTargets(["--slice", "simulator"]).map((target) => target.triple),
      ["aarch64-apple-ios-sim"],
    );
    assert.deepEqual(
      selectIosTargets(["--slice", "device"]).map((target) => target.triple),
      ["aarch64-apple-ios"],
    );
  });

  it("refuses a slice it does not know rather than building everything", () => {
    assert.throws(() => selectIosTargets(["--slice", "sim"]), /takes "device" or "simulator"/);
    assert.throws(() => selectIosTargets(["--slice"]), /takes "device" or "simulator"/);
  });

  it("only asks rustup for the targets being built", () => {
    const simulator = selectIosTargets(["--slice", "simulator"]);

    assert.deepEqual(missingTargets(["aarch64-apple-ios-sim"], simulator), []);
    assert.deepEqual(missingTargets([], simulator), ["aarch64-apple-ios-sim"]);
  });
});
