import { mkdtemp, mkdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { collectSizes, formatBytes, markdownTable, parseArgs } from "./package-size.mjs";

describe("package size report", () => {
  it("reads the target and profile in either flag form", () => {
    expect(parseArgs(["--target", "aarch64-apple-darwin", "--profile=debug"])).toEqual({
      profile: "debug",
      target: "aarch64-apple-darwin",
    });
    expect(parseArgs([])).toEqual({ profile: "release", target: null });
  });

  it("lists binaries, packages and the parts of a macOS app", async () => {
    const repoRoot = await mkdtemp(join(tmpdir(), "voyavpn-size-"));
    try {
      const release = join(repoRoot, "target", "release");
      const app = join(release, "bundle", "macos", "VoyaVPN.app");
      await mkdir(join(app, "Contents", "MacOS"), { recursive: true });
      await mkdir(join(release, "bundle", "dmg"), { recursive: true });
      await writeFile(join(release, "voyavpn"), Buffer.alloc(3 * 1024 * 1024));
      await writeFile(join(app, "Contents", "MacOS", "voyavpn"), Buffer.alloc(1024));
      await symlink("voyavpn", join(app, "Contents", "MacOS", "alias"));
      await writeFile(join(release, "bundle", "dmg", "VoyaVPN.dmg"), Buffer.alloc(2048));
      await writeFile(join(release, "bundle", "dmg", "bundle_dmg.sh"), "script");

      const rows = collectSizes(repoRoot, { profile: "release", target: "aarch64-apple-darwin" });

      expect(rows.map((row) => row.path)).toEqual([
        "target/release/voyavpn",
        "target/release/bundle/dmg/VoyaVPN.dmg",
        "target/release/bundle/macos/VoyaVPN.app",
        "  Contents/MacOS",
      ]);
      expect(rows[0].bytes).toBe(3 * 1024 * 1024);
      expect(markdownTable(rows)).toContain(`| \`target/release/voyavpn\` | ${formatBytes(3 * 1024 * 1024)} |`);
    } finally {
      await rm(repoRoot, { force: true, recursive: true });
    }
  });
});
