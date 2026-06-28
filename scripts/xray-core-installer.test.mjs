import { existsSync } from "node:fs";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it, vi } from "vitest";

import {
  defaultAppConfigDir,
  installXrayCore,
  shouldSkipXrayInstall,
  xrayAppExecutable,
  xrayAssetName,
  xraySeedDir,
} from "./xray-core-installer.mjs";

describe("xray core installer", () => {
  it("selects pinned upstream assets for supported platforms", () => {
    expect(xrayAssetName({ arch: "arm64", platform: "darwin" })).toBe("Xray-macos-arm64-v8a.zip");
    expect(xrayAssetName({ arch: "x64", platform: "linux" })).toBe("Xray-linux-64.zip");
    expect(xrayAssetName({ arch: "riscv64", platform: "darwin" })).toBeNull();
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
    expect(shouldSkipXrayInstall({ env: { VOYAVPN_SKIP_XRAY_POSTINSTALL: "1" }, postinstall: true })).toEqual({
      reason: "VOYAVPN_SKIP_XRAY_POSTINSTALL=1",
      skip: true,
    });
    expect(shouldSkipXrayInstall({ env: { CI: "true" }, postinstall: true })).toEqual({
      reason: "CI postinstall without VOYAVPN_FETCH_XRAY_ON_INSTALL=1",
      skip: true,
    });
    expect(
      shouldSkipXrayInstall({
        env: { CI: "true", VOYAVPN_FETCH_XRAY_ON_INSTALL: "1" },
        postinstall: true,
      }),
    ).toEqual({ reason: null, skip: false });
  });

  it("copies an existing seed into app data without fetching", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-xray-install-"));
    try {
      const repoRoot = join(workDir, "repo");
      const appConfigDir = join(workDir, "app-data");
      const seedDir = xraySeedDir(repoRoot);
      await mkdir(seedDir, { recursive: true });
      await writeFile(join(seedDir, "xray.exe"), "seed-xray");
      await writeFile(join(seedDir, "geoip.dat"), "seed-geo");

      const stageSeed = vi.fn(async () => {
        throw new Error("stageSeed should not be called");
      });
      const result = await installXrayCore({
        appConfigDir,
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === xrayAppExecutable(appConfigDir, "win32"),
        repoRoot,
        stageSeed,
      });

      expect(result.status).toBe("installed");
      expect(stageSeed).not.toHaveBeenCalled();
      expect(await readFile(xrayAppExecutable(appConfigDir, "win32"), "utf8")).toBe("seed-xray");
      expect(await readFile(join(appConfigDir, "bin", "xray", "geoip.dat"), "utf8")).toBe("seed-geo");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("backfills the seed directory from an existing app-data executable", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-xray-backfill-"));
    try {
      const repoRoot = join(workDir, "repo");
      const appConfigDir = join(workDir, "app-data");
      const appExecutable = xrayAppExecutable(appConfigDir, "win32");
      await mkdir(join(appConfigDir, "bin", "xray"), { recursive: true });
      await writeFile(appExecutable, "installed-xray");

      const result = await installXrayCore({
        appConfigDir,
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === appExecutable,
        repoRoot,
      });

      expect(result.status).toBe("already-installed");
      expect(await readFile(join(xraySeedDir(repoRoot), "xray.exe"), "utf8")).toBe("installed-xray");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("stages Xray when no seed executable exists", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-xray-stage-"));
    try {
      const repoRoot = join(workDir, "repo");
      const appConfigDir = join(workDir, "app-data");
      const stageSeed = vi.fn(async () => {
        const seedDir = xraySeedDir(repoRoot);
        await mkdir(seedDir, { recursive: true });
        await writeFile(join(seedDir, "xray.exe"), "downloaded-xray");
      });

      const result = await installXrayCore({
        appConfigDir,
        platform: "win32",
        probeExecutable: (path) => existsSync(path) && path === xrayAppExecutable(appConfigDir, "win32"),
        repoRoot,
        stageSeed,
      });

      expect(stageSeed).toHaveBeenCalledTimes(1);
      expect(result.status).toBe("installed");
      expect(await readFile(xrayAppExecutable(appConfigDir, "win32"), "utf8")).toBe("downloaded-xray");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });
});
