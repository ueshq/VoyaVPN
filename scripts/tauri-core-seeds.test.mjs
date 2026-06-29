import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { coreSeedBundleResources, hasExpectedSeedExecutable } from "./tauri-core-seeds.mjs";

describe("tauri core seed overlay", () => {
  it("requires the current platform executable before bundling sing-box seeds", async () => {
    const repoRoot = await mkdtemp(join(tmpdir(), "voyavpn-tauri-seeds-"));
    try {
      const singBoxDir = join(repoRoot, "src-tauri", "resources", "core-seeds", "sing_box");
      await mkdir(singBoxDir, { recursive: true });
      await writeFile(join(singBoxDir, "LICENSE"), "license");

      expect(hasExpectedSeedExecutable(singBoxDir, "linux")).toBe(false);
      expect(coreSeedBundleResources(repoRoot, { platform: "linux" })).toEqual({});

      await writeFile(join(singBoxDir, "sing-box"), "fake executable");

      expect(hasExpectedSeedExecutable(singBoxDir, "linux")).toBe(true);
      expect(coreSeedBundleResources(repoRoot, { platform: "linux" })).toEqual({
        "resources/core-seeds/sing_box/*": "core-seeds/sing_box/",
      });
    } finally {
      await rm(repoRoot, { force: true, recursive: true });
    }
  });
});
