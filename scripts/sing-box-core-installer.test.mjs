import { existsSync } from "node:fs";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it, vi } from "vitest";

import {
  defaultAppConfigDir,
  installSingBoxCore,
  shouldSkipSingBoxInstall,
  singBoxAppExecutable,
  singBoxAssetName,
  singBoxSeedDir,
} from "./sing-box-core-installer.mjs";

describe("sing-box core installer", () => {
  it("selects pinned upstream assets for supported platforms", () => {
    expect(singBoxAssetName({ arch: "arm64", platform: "darwin", version: "v1.13.14" })).toBe(
      "sing-box-1.13.14-darwin-arm64.tar.gz",
    );
    expect(singBoxAssetName({ arch: "x64", platform: "linux", version: "v1.13.14" })).toBe(
      "sing-box-1.13.14-linux-amd64.tar.gz",
    );
    expect(singBoxAssetName({ arch: "x64", platform: "win32", version: "v1.13.14" })).toBe(
      "sing-box-1.13.14-windows-amd64.zip",
    );
    expect(singBoxAssetName({ arch: "riscv64", platform: "darwin" })).toBeNull();
  });

  it("resolves app config directories and honors explicit overrides", () => {
    expect(defaultAppConfigDir({ env: { VOYAVPN_APP_CONFIG_DIR: "/tmp/custom-voya" }, platform: "darwin" })).toBe(
      "/tmp/custom-voya",
    );
    expect(defaultAppConfigDir({ env: {}, home: "/Users/tester", platform: "darwin" })).toBe(
      "/Users/tester/Library/Application Support/app.voyavpn.desktop",
    );
    expect(defaultAppConfigDir({ env: { XDG_CONFIG_HOME: "/home/tester/.config" }, platform: "linux" })).toBe(
      "/home/tester/.config/app.voyavpn.desktop",
    );
  });

  it("skips postinstall for explicit opt-out and CI", () => {
    expect(shouldSkipSingBoxInstall({ env: { VOYAVPN_SKIP_SING_BOX_POSTINSTALL: "1" }, postinstall: true })).toEqual({
      reason: "VOYAVPN_SKIP_SING_BOX_POSTINSTALL=1",
      skip: true,
    });
    expect(shouldSkipSingBoxInstall({ env: { CI: "true" }, postinstall: true })).toEqual({
      reason: "CI postinstall without VOYAVPN_FETCH_SING_BOX_ON_INSTALL=1",
      skip: true,
    });
    expect(
      shouldSkipSingBoxInstall({
        env: { CI: "true", VOYAVPN_FETCH_SING_BOX_ON_INSTALL: "1" },
        postinstall: true,
      }),
    ).toEqual({ reason: null, skip: false });
  });

  it("copies an existing seed into app data without fetching", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-install-"));
    try {
      const repoRoot = join(workDir, "repo");
      const appConfigDir = join(workDir, "app-data");
      const seedDir = singBoxSeedDir(repoRoot);
      await mkdir(seedDir, { recursive: true });
      await writeFile(join(seedDir, "sing-box.exe"), "seed-sing-box");
      await writeFile(join(seedDir, "LICENSE"), "seed-license");

      const stageSeed = vi.fn(async () => {
        throw new Error("stageSeed should not be called");
      });
      const result = await installSingBoxCore({
        appConfigDir,
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === singBoxAppExecutable(appConfigDir, "win32"),
        repoRoot,
        stageSeed,
      });

      expect(result.status).toBe("installed");
      expect(stageSeed).not.toHaveBeenCalled();
      expect(await readFile(singBoxAppExecutable(appConfigDir, "win32"), "utf8")).toBe("seed-sing-box");
      expect(await readFile(join(appConfigDir, "bin", "sing_box", "LICENSE"), "utf8")).toBe("seed-license");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("backfills the seed directory from an existing app-data executable", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-backfill-"));
    try {
      const repoRoot = join(workDir, "repo");
      const appConfigDir = join(workDir, "app-data");
      const appExecutable = singBoxAppExecutable(appConfigDir, "win32");
      await mkdir(join(appConfigDir, "bin", "sing_box"), { recursive: true });
      await writeFile(appExecutable, "installed-sing-box");

      const result = await installSingBoxCore({
        appConfigDir,
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === appExecutable,
        repoRoot,
      });

      expect(result.status).toBe("already-installed");
      expect(await readFile(join(singBoxSeedDir(repoRoot), "sing-box.exe"), "utf8")).toBe("installed-sing-box");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("stages sing-box when no seed executable exists", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-sing-box-stage-"));
    try {
      const repoRoot = join(workDir, "repo");
      const appConfigDir = join(workDir, "app-data");
      const stageSeed = vi.fn(async () => {
        const seedDir = singBoxSeedDir(repoRoot);
        await mkdir(seedDir, { recursive: true });
        await writeFile(join(seedDir, "sing-box.exe"), "downloaded-sing-box");
      });

      const result = await installSingBoxCore({
        appConfigDir,
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === singBoxAppExecutable(appConfigDir, "win32"),
        repoRoot,
        stageSeed,
      });

      expect(stageSeed).toHaveBeenCalledTimes(1);
      expect(result.status).toBe("installed");
      expect(await readFile(singBoxAppExecutable(appConfigDir, "win32"), "utf8")).toBe("downloaded-sing-box");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });
});
