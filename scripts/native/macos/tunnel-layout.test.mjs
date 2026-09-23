import { afterEach, describe, expect, it, vi } from "vitest";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

import {
  compareMacosVersions,
  distributionFromIdentityName,
  incompatiblePacketTunnelBundle,
  legacyPacketTunnelAppexBundle,
  libboxBinaryPath,
  packetTunnelLayout,
  packagingModeForDistribution,
  requiredNetworkExtensionValue,
  resolvePacketTunnelDeploymentTarget,
  resolvePacketTunnelVersions,
  resolveDmgPath,
} from "./tunnel-layout.mjs";

describe("Libbox framework layout", () => {
  let root;
  afterEach(() => {
    if (root) rmSync(root, { recursive: true, force: true });
  });

  it.each(["Libbox", "Versions/A/Libbox"])("finds the binary at %s", (relativePath) => {
    root = mkdtempSync(join(tmpdir(), "voya-libbox-layout-"));
    const binary = join(root, relativePath);
    mkdirSync(dirname(binary), { recursive: true });
    writeFileSync(binary, "fixture");
    expect(libboxBinaryPath(root)).toBe(binary);
  });

  it("prefers the framework root when both paths exist", () => {
    root = mkdtempSync(join(tmpdir(), "voya-libbox-layout-"));
    mkdirSync(join(root, "Versions", "A"), { recursive: true });
    writeFileSync(join(root, "Versions", "A", "Libbox"), "versioned");
    writeFileSync(join(root, "Libbox"), "root");
    expect(libboxBinaryPath(root)).toBe(join(root, "Libbox"));
  });
});

describe("macOS native tunnel layout", () => {
  const appContents = "/Applications/VoyaVPN.app/Contents";

  it("uses system extension packaging for Developer ID", () => {
    const layout = packetTunnelLayout(appContents, "developer-id");

    expect(packagingModeForDistribution("developer-id")).toBe("system-extension");
    expect(requiredNetworkExtensionValue("developer-id")).toBe("packet-tunnel-provider-systemextension");
    expect(layout.infoPackageType).toBe("SYSX");
    expect(layout.bundle).toBe(
      "/Applications/VoyaVPN.app/Contents/Library/SystemExtensions/app.voyavpn.desktop.PacketTunnel.systemextension",
    );
    expect(incompatiblePacketTunnelBundle(appContents, "developer-id")).toBe(
      "/Applications/VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex",
    );
  });

  it("uses app extension packaging for App Store and development lanes", () => {
    const layout = packetTunnelLayout(appContents, "app-store");

    expect(packagingModeForDistribution("app-store")).toBe("app-extension");
    expect(requiredNetworkExtensionValue("app-store")).toBe("packet-tunnel-provider");
    expect(layout.infoPackageType).toBe("XPC!");
    expect(layout.bundle).toBe("/Applications/VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex");
    // App Store validation: an appex folder is its executable name (ITMS-90362).
    expect(layout.binary).toBe(`${layout.bundle}/Contents/MacOS/VoyaPacketTunnel`);
    expect(legacyPacketTunnelAppexBundle(appContents)).toBe(
      "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
    );
  });

  it("compiles the PacketTunnel for its container's minimum macOS", () => {
    expect(resolvePacketTunnelDeploymentTarget({ appMinimumSystemVersion: "26.0", hostArch: "arm64" })).toEqual({
      minimumSystemVersion: "26.0",
      target: "arm64-apple-macos26.0",
    });
    // Apple Silicon starts at macOS 11, so an older container floor is raised for arm64 only.
    expect(resolvePacketTunnelDeploymentTarget({ appMinimumSystemVersion: "10.15", hostArch: "arm64" })).toEqual({
      minimumSystemVersion: "11.0",
      target: "arm64-apple-macos11.0",
    });
    expect(resolvePacketTunnelDeploymentTarget({ appMinimumSystemVersion: "10.15", hostArch: "x64" })).toEqual({
      minimumSystemVersion: "10.15",
      target: "x86_64-apple-macos10.15",
    });
    // Before the app bundle exists, tauri.conf.json's value stands in.
    expect(
      resolvePacketTunnelDeploymentTarget({ appMinimumSystemVersion: "", fallbackMinimumSystemVersion: "12.0", hostArch: "arm64" })
        .minimumSystemVersion,
    ).toBe("12.0");
    expect(() => resolvePacketTunnelDeploymentTarget({ hostArch: "arm64" })).toThrow(/no LSMinimumSystemVersion/u);
  });

  it("compares macOS versions numerically", () => {
    expect(compareMacosVersions("26.0", "12.0")).toBe(1);
    expect(compareMacosVersions("12", "12.0")).toBe(0);
    expect(compareMacosVersions("11.5", "12.0")).toBe(-1);
    expect(compareMacosVersions("10.15", "10.9")).toBe(1);
  });

  it("infers distribution from signing identity names", () => {
    expect(
      distributionFromIdentityName("Developer ID Application: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)", "auto"),
    ).toBe("developer-id");
    expect(distributionFromIdentityName("Apple Distribution: Example Team", "auto")).toBe("app-store");
    expect(distributionFromIdentityName("", "auto")).toBe("app-store");
  });
});

describe("PacketTunnel version fields", () => {
  // App Store Connect rejects an embedded extension whose version fields differ
  // from the containing app, so the container's Info.plist wins.
  it("takes both fields from the containing app when it is already built", () => {
    expect(
      resolvePacketTunnelVersions({
        appShortVersion: "1.4.2",
        appBundleVersion: "1.4.2.7",
        packageVersion: "0.1.0",
      }),
    ).toEqual({ build: "1.4.2.7", marketing: "1.4.2" });
  });

  it("falls back to the root package.json version before the app bundle exists", () => {
    expect(resolvePacketTunnelVersions({ packageVersion: "0.2.0" })).toEqual({
      build: "0.2.0",
      marketing: "0.2.0",
    });
    expect(resolvePacketTunnelVersions({ appShortVersion: "  ", appBundleVersion: "", packageVersion: "0.2.0" })).toEqual({
      build: "0.2.0",
      marketing: "0.2.0",
    });
  });

  it("mirrors the marketing version into CFBundleVersion when the app has none", () => {
    expect(resolvePacketTunnelVersions({ appShortVersion: "1.4.2", packageVersion: "0.1.0" })).toEqual({
      build: "1.4.2",
      marketing: "1.4.2",
    });
  });

  it("refuses to guess when no version is available at all", () => {
    expect(() => resolvePacketTunnelVersions({})).toThrow(/Unable to resolve a PacketTunnel version/u);
  });
});

describe("macOS DMG artifact paths", () => {
  const options = {
    appContents: "/Applications/VoyaVPN.app/Contents",
    dmgDir: "/tmp/voya-dmg",
    version: "1.2.3",
    env: {},
  };

  it.each([
    ["arm64", "aarch64"],
    ["x86_64", "x64"],
    ["x86_64 arm64\n", "universal"],
    ["", "aarch64"],
  ])("names the image from bundle architectures %j", (archs, suffix) => {
    const captureCommand = vi.fn()
      .mockReturnValueOnce({ stdout: "VoyaVPN\n" })
      .mockReturnValueOnce({ stdout: archs });
    expect(resolveDmgPath({ ...options, hostArch: "arm64", captureCommand }))
      .toBe(`/tmp/voya-dmg/VoyaVPN_1.2.3_${suffix}.dmg`);
    expect(captureCommand).toHaveBeenLastCalledWith(
      "lipo", ["-archs", "/Applications/VoyaVPN.app/Contents/MacOS/VoyaVPN"], { env: options.env },
    );
  });

  it("honors explicit path and architecture without reading the bundle", () => {
    const captureCommand = vi.fn();
    expect(resolveDmgPath({
      ...options, captureCommand,
      env: { VOYAVPN_MACOS_DMG_PATH: " /tmp/custom.dmg ", VOYAVPN_MACOS_DMG_ARCH: "ignored" },
    })).toBe("/tmp/custom.dmg");
    expect(resolveDmgPath({ ...options, captureCommand, env: { VOYAVPN_MACOS_DMG_ARCH: " universal " } }))
      .toBe("/tmp/voya-dmg/VoyaVPN_1.2.3_universal.dmg");
    expect(captureCommand).not.toHaveBeenCalled();
  });

  it("reports bundle inspection failures before selecting an artifact", () => {
    const captureCommand = vi.fn(() => { throw new Error("cannot inspect bundle"); });
    expect(() => resolveDmgPath({ ...options, captureCommand })).toThrow("cannot inspect bundle");
  });
});
