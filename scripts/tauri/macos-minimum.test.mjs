import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vite-plus/test";

import { compareMacosVersions } from "../native/macos/tunnel-layout.mjs";
import { appleSiliconMinimumSystemVersion, writeAppleSiliconMinimumOverlay } from "./macos-minimum.mjs";

describe("Apple Silicon minimum macOS", () => {
  const roots = [];
  afterEach(() => {
    for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
  });

  it("writes an overlay that only raises the minimum system version", () => {
    const repoRoot = mkdtempSync(join(tmpdir(), "voya-macos-minimum-"));
    roots.push(repoRoot);

    const overlayPath = writeAppleSiliconMinimumOverlay({ repoRoot });

    expect(overlayPath).toBe(join(repoRoot, "target", "release-config", "tauri.macos-arm64.generated.json"));
    expect(JSON.parse(readFileSync(overlayPath, "utf8"))).toEqual({
      bundle: { macOS: { minimumSystemVersion: "13.3" } },
    });
  });

  it("stays above the Intel floor tauri.conf.json declares", () => {
    const config = JSON.parse(
      readFileSync(new URL("../../apps/desktop/src-tauri/tauri.conf.json", import.meta.url), "utf8"),
    );
    const intelFloor = config.bundle.macOS.minimumSystemVersion;

    expect(intelFloor).toBe("12.3");
    expect(compareMacosVersions(appleSiliconMinimumSystemVersion, intelFloor)).toBeGreaterThan(0);
  });
});
