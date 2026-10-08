import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

import { androidArtifactPaths, hasSimulatorSlice, parseDevArgs } from "./dev.mjs";

describe("mobile dev lanes", () => {
  it("keeps its own flag and passes everything else to React Native", () => {
    expect(parseDevArgs(["--", "--rebuild-rust", "--simulator", "iPhone 17"])).toEqual({
      rebuildRust: true,
      reactNativeArgs: ["--simulator", "iPhone 17"],
    });
    expect(parseDevArgs([])).toEqual({ rebuildRust: false, reactNativeArgs: [] });
  });

  it("needs a simulator slice, not just any iOS slice", () => {
    expect(hasSimulatorSlice(["Info.plist", "ios-arm64", "ios-arm64-simulator"])).toBe(true);
    expect(hasSimulatorSlice(["Info.plist", "ios-arm64"])).toBe(false);
    expect(hasSimulatorSlice(null)).toBe(false);
  });

  it("looks for the Rust host on both ABIs, its Kotlin bindings, and Libbox", () => {
    const root = resolve("/repo/apps/mobile/android");
    expect(androidArtifactPaths(root)).toEqual({
      libbox: [resolve(root, "app/libs/libbox.aar")],
      rust: [
        resolve(root, "app/src/main/jniLibs/arm64-v8a/libvoya_mobile_ffi.so"),
        resolve(root, "app/src/main/jniLibs/x86_64/libvoya_mobile_ffi.so"),
        resolve(root, "app/src/main/java/uniffi/voya_mobile_ffi"),
      ],
    });
  });
});
