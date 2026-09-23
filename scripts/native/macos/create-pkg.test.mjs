import { describe, expect, it } from "vitest";

import { entitlementsEnableSandbox, pkgBuildPlan, resolvePkgPath, selectInstallerIdentity } from "./create-pkg.mjs";

const installerSha1 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const otherInstallerSha1 = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
const applicationSha1 = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
const installerName = "3rd Party Mac Developer Installer: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)";

describe("Mac App Store package", () => {
  it("builds the app as one /Applications component signed by the installer identity", () => {
    expect(
      pkgBuildPlan({ appBundle: "/b/VoyaVPN.app", installerIdentity: installerSha1, outputPath: "/out/VoyaVPN.pkg" }),
    ).toEqual([
      "--component",
      "/b/VoyaVPN.app",
      "/Applications",
      "--sign",
      installerSha1,
      "--timestamp",
      "/out/VoyaVPN.pkg",
    ]);
    expect(
      pkgBuildPlan({
        appBundle: "/b/VoyaVPN.app",
        installerIdentity: installerSha1,
        outputPath: "/out/VoyaVPN.pkg",
        disableTimestamp: true,
      }),
    ).not.toContain("--timestamp");
  });

  it("refuses to build an unsigned package", () => {
    expect(() => pkgBuildPlan({ appBundle: "/a", installerIdentity: " ", outputPath: "/o" })).toThrow(
      /Installer identity is required/,
    );
  });

  it("selects one installer identity even when the keychain lists it twice", () => {
    const identities = [
      { sha1: applicationSha1, name: "3rd Party Mac Developer Application: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)" },
      { sha1: installerSha1, name: installerName },
      { sha1: installerSha1, name: installerName },
    ];
    expect(selectInstallerIdentity(identities).sha1).toBe(installerSha1);
  });

  it("fails when no installer identity or several different ones match", () => {
    expect(() => selectInstallerIdentity([{ sha1: applicationSha1, name: "Apple Development: X (1)" }])).toThrow(
      /No 3rd Party Mac Developer Installer identity/,
    );

    const two = [
      { sha1: installerSha1, name: installerName },
      { sha1: otherInstallerSha1, name: installerName },
    ];
    expect(() => selectInstallerIdentity(two)).toThrow(/set VOYAVPN_INSTALLER_IDENTITY/);
    expect(selectInstallerIdentity(two, otherInstallerSha1.toLowerCase()).sha1).toBe(otherInstallerSha1);
  });

  it("recognizes executables that enable App Sandbox", () => {
    expect(
      entitlementsEnableSandbox(
        "<dict>\n\t<key>com.apple.security.app-sandbox</key>\n\t<true/>\n\t<key>com.apple.security.inherit</key>\n\t<true/>\n</dict>",
      ),
    ).toBe(true);
    expect(entitlementsEnableSandbox("<key>com.apple.security.app-sandbox</key><false/>")).toBe(false);
    expect(entitlementsEnableSandbox("")).toBe(false);
  });

  it("names the package after version, build and architecture unless overridden", () => {
    expect(resolvePkgPath({ pkgDir: "/out", version: "0.1.0", buildNumber: "412", arch: "aarch64", env: {} })).toBe(
      "/out/VoyaVPN_0.1.0_412_aarch64.pkg",
    );
    expect(
      resolvePkgPath({
        pkgDir: "/out",
        version: "0.1.0",
        buildNumber: "412",
        arch: "aarch64",
        env: { VOYAVPN_MACOS_PKG_PATH: "/tmp/x.pkg" },
      }),
    ).toBe("/tmp/x.pkg");
  });
});
