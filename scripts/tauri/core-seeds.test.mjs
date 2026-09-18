import { spawnSync } from "node:child_process";
import { readFileSync, statSync, utimesSync } from "node:fs";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, relative, resolve, sep } from "node:path";
import { describe, expect, it } from "vitest";

import { repoRootFromScript } from "../lib/common.mjs";
import {
  coreSeedBundleResources,
  hasExpectedSeedExecutable,
  requiredBundleResources,
  writeOptionalCoreSeedOverlay,
} from "./core-seeds.mjs";

const repoRoot = repoRootFromScript(import.meta.url);

describe("tauri core seed overlay", () => {
  it("restates tauri.conf.json's bundle resources, and every one is tracked by git", () => {
    const tauriDir = join(repoRoot, "apps", "desktop", "src-tauri");
    const config = JSON.parse(readFileSync(join(tauriDir, "tauri.conf.json"), "utf8"));

    expect(requiredBundleResources).toEqual(config.bundle.resources);

    // An ignored or untracked resource exists on the machine that added it and
    // nowhere else: the Tauri build script then fails in every clean checkout,
    // CI included, which is how CI stayed red after docs/ was ignored.
    for (const source of Object.keys(requiredBundleResources)) {
      const path = relative(repoRoot, resolve(tauriDir, source)).split(sep).join("/");
      const tracked = spawnSync("git", ["ls-files", "--error-unmatch", "--", path], { cwd: repoRoot });
      expect(tracked.status, `${path} must be tracked by git`).toBe(0);
    }
  });

  it("requires the current platform executable before bundling sing-box seeds", async () => {
    const repoRoot = await mkdtemp(join(tmpdir(), "voyavpn-tauri-seeds-"));
    try {
      const singBoxDir = join(repoRoot, "apps", "desktop", "src-tauri", "resources", "core-seeds", "sing_box");
      await mkdir(singBoxDir, { recursive: true });
      await writeFile(join(singBoxDir, "LICENSE"), "license");

      expect(hasExpectedSeedExecutable(singBoxDir, "linux")).toBe(false);
      expect(coreSeedBundleResources(repoRoot, { platform: "linux" })).toEqual({});

      await writeFile(join(singBoxDir, "sing-box"), "fake executable");

      expect(hasExpectedSeedExecutable(singBoxDir, "linux")).toBe(true);
      expect(coreSeedBundleResources(repoRoot, { platform: "linux" })).toEqual({
        "resources/core-seeds/sing_box/*": "core-seeds/sing_box/",
      });

      // macOS bundles the seed too: it measures nodes while disconnected.
      expect(coreSeedBundleResources(repoRoot, { platform: "darwin" })).toEqual({
        "resources/core-seeds/sing_box/*": "core-seeds/sing_box/",
      });
    } finally {
      await rm(repoRoot, { force: true, recursive: true });
    }
  });

  it("rewrites the overlay only when its content changes", async () => {
    const repoRoot = await mkdtemp(join(tmpdir(), "voyavpn-tauri-overlay-"));
    try {
      const singBoxDir = join(repoRoot, "apps", "desktop", "src-tauri", "resources", "core-seeds", "sing_box");
      await mkdir(singBoxDir, { recursive: true });
      await writeFile(join(singBoxDir, "sing-box"), "fake executable");
      const overlayPath = join(repoRoot, "target", "tauri-config", "tauri.core-seeds.generated.json");

      expect(writeOptionalCoreSeedOverlay(repoRoot, overlayPath, { platform: "linux" })).toBe(overlayPath);
      const written = readFileSync(overlayPath, "utf8");
      expect(JSON.parse(written).bundle.resources).toEqual({
        ...requiredBundleResources,
        "resources/core-seeds/sing_box/*": "core-seeds/sing_box/",
      });

      // Backdate the file so an unnecessary rewrite would show up as a new mtime.
      const past = new Date("2020-01-01T00:00:00Z");
      utimesSync(overlayPath, past, past);
      writeOptionalCoreSeedOverlay(repoRoot, overlayPath, { platform: "linux" });
      expect(statSync(overlayPath).mtimeMs).toBe(past.getTime());

      await writeFile(overlayPath, "{}\n");
      writeOptionalCoreSeedOverlay(repoRoot, overlayPath, { platform: "linux" });
      expect(readFileSync(overlayPath, "utf8")).toBe(written);
    } finally {
      await rm(repoRoot, { force: true, recursive: true });
    }
  });
});
