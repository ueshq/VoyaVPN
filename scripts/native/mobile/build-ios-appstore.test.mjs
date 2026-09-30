import { describe, expect, it } from "vitest";

import {
  builtInfoProblems,
  entitlementProblems,
  exportOptionsPlist,
  iosProfileProblems,
  resolveIpaPath,
  signingBuildSettings,
} from "./build-ios-appstore.mjs";
import { parsePlist } from "./ios-bundle-checks.mjs";

const teamId = "4LUKJ56532";
const storeProfile = (bundleId, overrides = {}) => ({
  name: `${bundleId} App Store`,
  uuid: `uuid-${bundleId}`,
  applicationIdentifier: `${teamId}.${bundleId}`,
  bundleIdentifier: bundleId,
  teamIdentifier: teamId,
  appGroups: ["group.app.voyavpn.mobile"],
  networkExtensions: ["packet-tunnel-provider", "app-proxy-provider"],
  developerCertificateSubjects: ["CN=Apple Distribution: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)"],
  developerCertificateFingerprints: ["aa"],
  getTaskAllow: false,
  provisionedDevices: null,
  ...overrides,
});
const appProfile = storeProfile("app.voyavpn.mobile");
const tunnelProfile = storeProfile("app.voyavpn.mobile.PacketTunnel");
const identityName = "Apple Distribution: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)";

describe("iOS App Store provisioning profiles", () => {
  it("accepts a store profile that grants the tunnel and the App Group", () => {
    expect(iosProfileProblems(appProfile, "app.voyavpn.mobile")).toEqual([]);
  });

  // What Xcode creates on its own, and all that exists before the store profiles are made.
  it("rejects Xcode's development profile", () => {
    const development = storeProfile("app.voyavpn.mobile", {
      name: "iOS Team Provisioning Profile: app.voyavpn.mobile",
      developerCertificateSubjects: ["CN=Apple Development: Vincent Zhang (5357LGMALT)"],
      getTaskAllow: true,
      provisionedDevices: ["00008130-000000000000001E"],
    });
    const problems = iosProfileProblems(development, "app.voyavpn.mobile");
    expect(problems).toHaveLength(2);
    expect(problems[0]).toMatch(/not an App Store distribution profile/u);
    expect(problems[1]).toMatch(/get-task-allow/u);
  });

  it("rejects the wrong app, a missing App Group and a missing capability", () => {
    const desktopGroup = storeProfile("app.voyavpn.mobile", {
      appGroups: ["group.app.voyavpn.desktop"],
      networkExtensions: ["app-proxy-provider"],
    });
    expect(iosProfileProblems(desktopGroup, "app.voyavpn.mobile")).toEqual([
      'Profile "app.voyavpn.mobile App Store" does not grant the App Group group.app.voyavpn.mobile.',
      'Profile "app.voyavpn.mobile App Store" does not grant Network Extensions packet-tunnel-provider.',
    ]);
    expect(iosProfileProblems(appProfile, "app.voyavpn.mobile.PacketTunnel")[0]).toMatch(
      /is for 4LUKJ56532\.app\.voyavpn\.mobile, expected 4LUKJ56532\.app\.voyavpn\.mobile\.PacketTunnel/u,
    );
  });
});

describe("signed entitlements", () => {
  const target = { name: "VoyaVPN.app", bundleId: "app.voyavpn.mobile", teamId };
  const store = {
    "application-identifier": "4LUKJ56532.app.voyavpn.mobile",
    "com.apple.developer.team-identifier": teamId,
    "com.apple.developer.networking.networkextension": ["packet-tunnel-provider"],
    "com.apple.security.application-groups": ["group.app.voyavpn.mobile"],
    "beta-reports-active": true,
  };

  it("accepts a store signature, TestFlight flag included", () => {
    expect(entitlementProblems(store, target)).toEqual([]);
  });

  it("rejects a development signature", () => {
    expect(entitlementProblems({ ...store, "get-task-allow": true }, target)).toEqual([
      "VoyaVPN.app: get-task-allow is set; the bundle was signed for development.",
    ]);
  });

  it("rejects capabilities the app does not use and a wrong identifier", () => {
    const problems = entitlementProblems(
      {
        ...store,
        "application-identifier": "4LUKJ56532.app.voyavpn.desktop",
        "com.apple.developer.networking.networkextension": ["packet-tunnel-provider", "dns-proxy"],
        "aps-environment": "production",
      },
      target,
    );
    expect(problems).toHaveLength(3);
    expect(problems[0]).toMatch(/application-identifier is "4LUKJ56532\.app\.voyavpn\.desktop"/u);
    expect(problems[1]).toMatch(/networkextension is \["packet-tunnel-provider","dns-proxy"\]/u);
    expect(problems[2]).toBe("VoyaVPN.app: unexpected entitlement aps-environment.");
  });
});

describe("built Info.plist fields", () => {
  const info = (bundleId, overrides = {}) => ({
    bundleId,
    shortVersion: "0.1.0",
    bundleVersion: "412",
    minimumOsVersion: "15.1",
    hasIcons: true,
    ...overrides,
  });
  const expected = { version: "0.1.0", buildNumber: "412" };

  it("accepts an app and extension with one version and build number", () => {
    expect(
      builtInfoProblems({ app: info("app.voyavpn.mobile"), tunnel: info("app.voyavpn.mobile.PacketTunnel"), ...expected }),
    ).toEqual([]);
  });

  // The project's own build number is 1; only the command-line override reaches both targets.
  it("rejects an extension left at the project's build number, and a missing icon", () => {
    expect(
      builtInfoProblems({
        app: info("app.voyavpn.mobile", { hasIcons: false }),
        tunnel: info("app.voyavpn.mobile.PacketTunnel", { bundleVersion: "1" }),
        ...expected,
      }),
    ).toEqual([
      "PacketTunnel.appex CFBundleVersion is 1, expected 412.",
      "VoyaVPN.app has no CFBundleIcons; the icon set was not compiled in.",
    ]);
  });
});

describe("xcodebuild inputs", () => {
  const signing = { teamId, identityName, appProfile, tunnelProfile };

  it("picks each target's profile by product name", () => {
    expect(signingBuildSettings(signing)).toEqual([
      "CODE_SIGN_STYLE=Manual",
      "DEVELOPMENT_TEAM=4LUKJ56532",
      `CODE_SIGN_IDENTITY=${identityName}`,
      "PROVISIONING_PROFILE_SPECIFIER=$(VOYA_PROFILE_$(PRODUCT_NAME))",
      "VOYA_PROFILE_VoyaVPN=uuid-app.voyavpn.mobile",
      "VOYA_PROFILE_PacketTunnel=uuid-app.voyavpn.mobile.PacketTunnel",
    ]);
  });

  it("writes export options for an App Store Connect upload with both profiles", () => {
    expect(parsePlist(exportOptionsPlist(signing))).toEqual({
      method: "app-store-connect",
      destination: "export",
      teamID: teamId,
      signingStyle: "manual",
      signingCertificate: "Apple Distribution",
      provisioningProfiles: {
        "app.voyavpn.mobile": "uuid-app.voyavpn.mobile",
        "app.voyavpn.mobile.PacketTunnel": "uuid-app.voyavpn.mobile.PacketTunnel",
      },
      manageAppVersionAndBuildNumber: false,
      uploadSymbols: true,
      stripSwiftSymbols: true,
    });
  });

  it("names the package after version and build number", () => {
    expect(resolveIpaPath({ outputDir: "/out", version: "0.1.0", buildNumber: "412" })).toBe("/out/VoyaVPN_0.1.0_412.ipa");
  });
});
