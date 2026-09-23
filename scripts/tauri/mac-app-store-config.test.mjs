import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  macAppStoreOverlay,
  requestedMacAppStoreBuild,
  resolveMacAppStoreBuildNumber,
  writeMacAppStoreOverlay,
} from "./mac-app-store-config.mjs";

const gitCount = (stdout, status = 0) => vi.fn(() => ({ status, stdout, stderr: "" }));

describe("Mac App Store Tauri config", () => {
  const roots = [];
  afterEach(() => {
    for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
  });

  it("is requested only by an explicit VOYAVPN_MAC_APP_STORE", () => {
    expect(requestedMacAppStoreBuild({ VOYAVPN_MAC_APP_STORE: "1" })).toBe(true);
    expect(requestedMacAppStoreBuild({ VOYAVPN_MAC_APP_STORE: "false" })).toBe(false);
    expect(requestedMacAppStoreBuild({ VOYAVPN_MACOS_DISTRIBUTION: "app-store" })).toBe(false);
  });

  it("prefers an explicit build number and validates its shape", () => {
    const captureCommand = gitCount("999\n");
    expect(
      resolveMacAppStoreBuildNumber({ env: { VOYAVPN_MACOS_BUILD_NUMBER: " 12.3 " }, repoRoot: "/repo", captureCommand }),
    ).toBe("12.3");
    expect(captureCommand).not.toHaveBeenCalled();

    for (const invalid of ["1.2.3.4", "v12", "12-rc", "1..2"]) {
      expect(() =>
        resolveMacAppStoreBuildNumber({ env: { VOYAVPN_MACOS_BUILD_NUMBER: invalid }, repoRoot: "/repo", captureCommand }),
      ).toThrow(/VOYAVPN_MACOS_BUILD_NUMBER/);
    }
  });

  it("falls back to the commit count and fails when git cannot answer", () => {
    const captureCommand = gitCount("412\n");
    expect(resolveMacAppStoreBuildNumber({ env: {}, repoRoot: "/repo", captureCommand })).toBe("412");
    expect(captureCommand).toHaveBeenCalledWith("git", ["rev-list", "--count", "HEAD"], { cwd: "/repo" });

    expect(() => resolveMacAppStoreBuildNumber({ env: {}, repoRoot: "/repo", captureCommand: gitCount("", 128) })).toThrow(
      /set VOYAVPN_MACOS_BUILD_NUMBER/,
    );
  });

  it("bundles only the app, for macOS 26+, with the build number and no updater artifacts", () => {
    expect(macAppStoreOverlay({ buildNumber: "7" })).toEqual({
      bundle: {
        targets: ["app"],
        createUpdaterArtifacts: false,
        macOS: { minimumSystemVersion: "26.0", bundleVersion: "7" },
      },
    });
  });

  it("writes the overlay under target/release-config", () => {
    const repoRoot = mkdtempSync(join(tmpdir(), "voya-mas-"));
    roots.push(repoRoot);

    const { overlayPath, buildNumber } = writeMacAppStoreOverlay({
      repoRoot,
      env: { VOYAVPN_MACOS_BUILD_NUMBER: "30" },
    });

    expect(buildNumber).toBe("30");
    expect(overlayPath).toBe(join(repoRoot, "target", "release-config", "tauri.mac-app-store.generated.json"));
    expect(JSON.parse(readFileSync(overlayPath, "utf8")).bundle.macOS.bundleVersion).toBe("30");
  });
});
