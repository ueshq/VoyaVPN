import { describe, expect, it } from "vitest";

import {
  appexInfoProblems,
  deploymentTargetProblems,
  entitlementsEnableSandbox,
  parseMachOMinimumVersions,
  pkgBuildPlan,
  resolvePkgPath,
  seedOriginProblems,
  selectInstallerIdentity,
} from "./create-pkg.mjs";

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

describe("App Store validation rules checked before packaging", () => {
  const otoolBuildVersion = (minos) => `Load command 10
      cmd LC_BUILD_VERSION
  cmdsize 32
 platform 1
    minos ${minos}
      sdk 26.5
   ntools 1
     tool 3
  version 1267.0
Load command 11
      cmd LC_SOURCE_VERSION`;

  it("reads each slice's deployment target from otool output, ignoring tool versions", () => {
    expect(parseMachOMinimumVersions(otoolBuildVersion("26.0"))).toEqual(["26.0"]);
    expect(
      parseMachOMinimumVersions(`Load command 9
      cmd LC_VERSION_MIN_MACOSX
  cmdsize 16
  version 10.15
      sdk 14.0`),
    ).toEqual(["10.15"]);
    expect(parseMachOMinimumVersions("")).toEqual([]);
  });

  // The upload that failed with ITMS-90869: arm64 only, declared 11.0, with a
  // PacketTunnel and seed built for the host's macOS 26.
  it("rejects the bundle Transporter rejected", () => {
    const problems = deploymentTargetProblems({
      appMinimumSystemVersion: "11.0",
      arm64Only: true,
      executables: [
        { name: "MacOS/voyavpn", minimumVersions: ["11.0"] },
        { name: "PlugIns/VoyaPacketTunnel.appex/Contents/MacOS/VoyaPacketTunnel", minimumVersions: ["26.0"] },
        { name: "Resources/core-seeds/sing_box/sing-box", minimumVersions: ["26.0"] },
      ],
    });

    expect(problems).toHaveLength(3);
    expect(problems[0]).toMatch(/12\.0 or later.*ITMS-90869/u);
    expect(problems[1]).toMatch(/VoyaPacketTunnel is built for macOS 26\.0, newer than the app's 11\.0/u);
    expect(problems[2]).toMatch(/sing-box is built for macOS 26\.0/u);
  });

  it("accepts a macOS 26 arm64 bundle and a lower floor for universal builds", () => {
    expect(
      deploymentTargetProblems({
        appMinimumSystemVersion: "26.0",
        arm64Only: true,
        executables: [{ name: "MacOS/voyavpn", minimumVersions: ["26.0"] }],
      }),
    ).toEqual([]);
    expect(
      deploymentTargetProblems({
        appMinimumSystemVersion: "11.0",
        arm64Only: false,
        executables: [{ name: "MacOS/voyavpn", minimumVersions: ["11.0", "10.15"] }],
      }),
    ).toEqual([]);
    expect(deploymentTargetProblems({ appMinimumSystemVersion: "", arm64Only: true, executables: [] })).toEqual([
      "The app's Info.plist declares no LSMinimumSystemVersion.",
    ]);
  });

  it("rejects the PacketTunnel shape behind ITMS-90360 and ITMS-90362", () => {
    const problems = appexInfoProblems({
      folderName: "app.voyavpn.desktop.PacketTunnel.appex",
      executableName: "VoyaPacketTunnel",
      minimumSystemVersion: "",
      appMinimumSystemVersion: "26.0",
    });

    expect(problems).toHaveLength(2);
    expect(problems[0]).toMatch(/must equal the folder name, "app\.voyavpn\.desktop\.PacketTunnel" \(ITMS-90362\)/u);
    expect(problems[1]).toMatch(/no LSMinimumSystemVersion \(ITMS-90360\)/u);
  });

  it("accepts the renamed PacketTunnel and flags a floor that drifted from the app", () => {
    const appex = {
      folderName: "VoyaPacketTunnel.appex",
      executableName: "VoyaPacketTunnel",
      appMinimumSystemVersion: "26.0",
    };
    expect(appexInfoProblems({ ...appex, minimumSystemVersion: "26.0" })).toEqual([]);
    expect(appexInfoProblems({ ...appex, minimumSystemVersion: "11.0" })[0]).toMatch(/differs from the app's 26\.0/u);
  });

  it("requires the source-built sing-box seed", () => {
    expect(seedOriginProblems({ origin: "source", tags: ["with_quic", "with_clash_api"] })).toEqual([]);
    expect(seedOriginProblems(null)).toEqual(["The bundled sing-box seed has no sing-box.seed.json."]);
    expect(seedOriginProblems({ assetName: "sing-box-1.13.14-darwin-arm64.tar.gz" })).toEqual([
      "The bundled sing-box seed is the upstream build; the store package needs the source-built one (pnpm core:sing-box:build).",
    ]);
    expect(seedOriginProblems({ origin: "source", tags: ["with_quic", "with_naive_outbound"] })).toEqual([
      "The bundled sing-box seed was built with with_naive_outbound.",
    ]);
  });
});
