import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { coreSeedBundleResources, hasExpectedSeedExecutable } from "./tauri-core-seeds.mjs";

describe("tauri core seed overlay", () => {
  it("requires the current platform executable before bundling xray seeds", async () => {
    const repoRoot = await mkdtemp(join(tmpdir(), "voyavpn-tauri-seeds-"));
    try {
      const xrayDir = join(repoRoot, "src-tauri", "resources", "core-seeds", "xray");
      await mkdir(xrayDir, { recursive: true });
      await writeFile(join(xrayDir, "geoip.dat"), "geo");

      expect(hasExpectedSeedExecutable(xrayDir, "linux")).toBe(false);
      expect(coreSeedBundleResources(repoRoot, { platform: "linux" })).toEqual({});

      await writeFile(join(xrayDir, "xray"), "fake executable");

      expect(hasExpectedSeedExecutable(xrayDir, "linux")).toBe(true);
      expect(coreSeedBundleResources(repoRoot, { platform: "linux" })).toEqual({
        "resources/core-seeds/xray/*": "core-seeds/xray/",
      });
    } finally {
      await rm(repoRoot, { force: true, recursive: true });
    }
  });
});
