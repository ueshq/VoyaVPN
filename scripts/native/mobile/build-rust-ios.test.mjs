import assert from "node:assert/strict";
import { describe, it } from "vitest";

import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { IOS_TARGETS, missingTargets, selectIosTargets, syncGeneratedFiles } from "./build-rust-ios.mjs";
import { rustProfile } from "./uniffi-bindings.mjs";

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
    assert.deepEqual(
      selectIosTargets(["--", "--slice=device"]).map((target) => target.triple),
      ["aarch64-apple-ios"],
    );
  });

  it("refuses a slice it does not know rather than building everything", () => {
    assert.throws(() => selectIosTargets(["--slice", "sim"]), /takes "device" or "simulator"/);
    assert.throws(() => selectIosTargets(["--slice"]), /--slice requires a value/);
    assert.throws(() => selectIosTargets(["--slices", "device"]), /Unknown argument: --slices/);
  });

  it("names the Cargo profile and the directory its output lands in", () => {
    assert.deepEqual(rustProfile("release"), { directory: "release", profile: "release" });
    assert.deepEqual(rustProfile("mobile-smoke"), { directory: "mobile-smoke", profile: "mobile-smoke" });
    // `dev` is the one profile whose directory has another name.
    assert.deepEqual(rustProfile("debug"), { directory: "debug", profile: "dev" });
    assert.deepEqual(rustProfile("dev"), { directory: "debug", profile: "dev" });
  });

  it("only asks rustup for the targets being built", () => {
    const simulator = selectIosTargets(["--slice", "simulator"]);

    assert.deepEqual(missingTargets(["aarch64-apple-ios-sim"], simulator), []);
    assert.deepEqual(missingTargets([], simulator), ["aarch64-apple-ios-sim"]);
  });

  it("leaves an unchanged binding untouched so Xcode does not recompile it", () => {
    const root = mkdtempSync(join(tmpdir(), "voya-sync-"));
    try {
      const fresh = join(root, "fresh");
      const generated = join(root, "generated");
      for (const directory of [fresh, generated]) mkdirSync(directory);
      writeFileSync(join(fresh, "same.h"), "same");
      writeFileSync(join(fresh, "changed.swift"), "new");
      writeFileSync(join(generated, "same.h"), "same");
      writeFileSync(join(generated, "changed.swift"), "old");
      writeFileSync(join(generated, "stale.swift"), "gone");
      const past = new Date(1_000_000_000_000);
      utimesSync(join(generated, "same.h"), past, past);

      syncGeneratedFiles(fresh, generated);

      assert.deepEqual(readdirSync(generated).sort(), ["changed.swift", "same.h"]);
      assert.equal(readFileSync(join(generated, "changed.swift"), "utf8"), "new");
      assert.equal(statSync(join(generated, "same.h")).mtimeMs, past.getTime());
    } finally {
      rmSync(root, { force: true, recursive: true });
    }
  });
});
