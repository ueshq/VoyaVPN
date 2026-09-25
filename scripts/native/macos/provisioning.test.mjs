import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { capture } from "../../lib/common.mjs";

import {
  assertProfileCapabilities,
  certificateSha1Fingerprint,
  embedProvisioningProfile,
  findMatchingIdentities,
  formatProfileSelectionError,
  inferDistribution,
  parseCodesigningIdentities,
  profileContainsCertificate,
  profileDeviceCoverage,
  profileMatchesDistribution,
  profileRejectionReason,
  isStoreDistributionProfile,
  renderProfileEntitlements,
  selectProvisioningProfile,
  signedNetworkExtensions,
  validateProvisioningProfile,
} from "./provisioning.mjs";

const developmentSha1 = "1111111111111111111111111111111111111111";
const distributionSha1 = "2222222222222222222222222222222222222222";
const developerIdSha1 = "3333333333333333333333333333333333333333";
const localUdid = "00006021-001248AC0AF0C01E";

const findIdentityOutput = `  1) ${distributionSha1.toUpperCase()} "3rd Party Mac Developer Application: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)"
  2) ${developerIdSha1.toUpperCase()} "Developer ID Application: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)"
  3) ${developmentSha1.toUpperCase()} "Apple Development: Example Developer (ABCDE12345)"
     3 valid identities found`;

function profileFixture(overrides = {}) {
  return {
    path: "/certs/example.provisionprofile",
    name: "example",
    bundleIdentifier: "app.voyavpn.desktop",
    applicationIdentifier: "4LUKJ56532.app.voyavpn.desktop",
    teamIdentifier: "4LUKJ56532",
    appGroups: ["group.app.voyavpn.desktop"],
    networkExtensions: ["packet-tunnel-provider"],
    developerCertificateSubjects: [],
    developerCertificateFingerprints: [],
    provisionsAllDevices: false,
    provisionedDevices: null,
    expirationDate: "2044-06-28T02:08:36Z",
    ...overrides,
  };
}

function developmentProfile(bundleIdentifier) {
  return profileFixture({
    path: `/certs/${bundleIdentifier}-dev.provisionprofile`,
    name: `${bundleIdentifier} development`,
    bundleIdentifier,
    developerCertificateSubjects: ["/UID=4LUKJ56532/CN=Apple Development: Example Developer (ABCDE12345)"],
    developerCertificateFingerprints: [developmentSha1.toUpperCase()],
    provisionedDevices: [localUdid],
  });
}

function appStoreProfile(bundleIdentifier) {
  return profileFixture({
    path: `/certs/${bundleIdentifier}.provisionprofile`,
    name: `${bundleIdentifier} app store`,
    bundleIdentifier,
    developerCertificateSubjects: [
      "/UID=4LUKJ56532/CN=3rd Party Mac Developer Application: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)",
    ],
    developerCertificateFingerprints: [distributionSha1.toUpperCase()],
  });
}

function developerIdProfile(bundleIdentifier) {
  return profileFixture({
    path: `/certs/${bundleIdentifier}-developer-id.provisionprofile`,
    name: `${bundleIdentifier} developer id`,
    bundleIdentifier,
    developerCertificateSubjects: [
      "/UID=4LUKJ56532/CN=Developer ID Application: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)",
    ],
    developerCertificateFingerprints: [developerIdSha1.toUpperCase()],
    networkExtensions: ["packet-tunnel-provider-systemextension"],
    provisionsAllDevices: true,
  });
}

function sixProfileDirectory() {
  return [
    appStoreProfile("app.voyavpn.desktop"),
    appStoreProfile("app.voyavpn.desktop.PacketTunnel"),
    developerIdProfile("app.voyavpn.desktop"),
    developerIdProfile("app.voyavpn.desktop.PacketTunnel"),
    developmentProfile("app.voyavpn.desktop"),
    developmentProfile("app.voyavpn.desktop.PacketTunnel"),
  ];
}

describe("parseCodesigningIdentities", () => {
  it("parses security find-identity output", () => {
    const identities = parseCodesigningIdentities(findIdentityOutput);
    expect(identities).toHaveLength(3);
    expect(identities[1]).toEqual({
      sha1: developerIdSha1.toUpperCase(),
      name: "Developer ID Application: Beijing Wangcai Technology Co., Ltd. (4LUKJ56532)",
      error: null,
    });
  });

  it("ignores the trailing summary line", () => {
    expect(parseCodesigningIdentities("     0 valid identities found")).toEqual([]);
  });

  it("parses invalid identities annotated with CSSMERR codes", () => {
    const identities = parseCodesigningIdentities(
      `  1) ${developmentSha1.toUpperCase()} "Apple Development: Example Developer (ABCDE12345)" (CSSMERR_TP_NOT_TRUSTED)`,
    );
    expect(identities).toHaveLength(1);
    expect(identities[0].name).toBe("Apple Development: Example Developer (ABCDE12345)");
    expect(identities[0].error).toBe("CSSMERR_TP_NOT_TRUSTED");
  });
});

describe("findMatchingIdentities", () => {
  const identities = parseCodesigningIdentities(findIdentityOutput);

  it("matches by SHA-1 case-insensitively", () => {
    expect(findMatchingIdentities(identities, developmentSha1.toLowerCase())).toHaveLength(1);
  });

  it("matches by substring and regex", () => {
    expect(findMatchingIdentities(identities, "Developer ID Application")).toHaveLength(1);
    expect(findMatchingIdentities(identities, /Apple Development:|Mac Developer:/)).toHaveLength(1);
  });

  it("returns no matches for an empty spec", () => {
    expect(findMatchingIdentities(identities, "")).toEqual([]);
  });
});

describe("certificateSha1Fingerprint", () => {
  it("returns the uppercase SHA-1 of the DER bytes", () => {
    expect(certificateSha1Fingerprint(Buffer.from("test"))).toBe("A94A8FE5CCB19BA61C4C0873D391E987982FBBD3");
  });
});

describe("profileMatchesDistribution", () => {
  it("accepts development certificates for app-store only when development provisioning is allowed", () => {
    const profile = developmentProfile("app.voyavpn.desktop");
    expect(profileMatchesDistribution(profile, "app-store", false)).toBe(false);
    expect(profileMatchesDistribution(profile, "app-store", true)).toBe(true);
  });

  it("requires Developer ID certificates for developer-id", () => {
    expect(profileMatchesDistribution(developerIdProfile("app.voyavpn.desktop"), "developer-id")).toBe(true);
    expect(profileMatchesDistribution(developmentProfile("app.voyavpn.desktop"), "developer-id")).toBe(false);
  });
});

describe("profileContainsCertificate", () => {
  it("matches fingerprints case-insensitively and rejects missing identities", () => {
    const profile = developmentProfile("app.voyavpn.desktop");
    expect(profileContainsCertificate(profile, developmentSha1.toLowerCase())).toBe(true);
    expect(profileContainsCertificate(profile, distributionSha1)).toBe(false);
    expect(profileContainsCertificate(profile, "")).toBe(false);
  });
});

describe("profileDeviceCoverage", () => {
  it("distinguishes all-devices, listed, not-listed, no-device-list, and unknown", () => {
    expect(profileDeviceCoverage(developerIdProfile("a"), localUdid)).toBe("all-devices");
    expect(profileDeviceCoverage(developmentProfile("a"), localUdid.toLowerCase())).toBe("listed");
    expect(profileDeviceCoverage(developmentProfile("a"), "00000000-DEADBEEF00000000")).toBe("not-listed");
    expect(profileDeviceCoverage(appStoreProfile("a"), localUdid)).toBe("no-device-list");
    expect(profileDeviceCoverage(developmentProfile("a"), null)).toBe("unknown");
  });
});

describe("profileRejectionReason", () => {
  it("rejects expired profiles", () => {
    const profile = developmentProfile("app.voyavpn.desktop");
    const reason = profileRejectionReason(
      { ...profile, expirationDate: "2020-01-01T00:00:00Z" },
      {
        bundleIdentifier: "app.voyavpn.desktop",
        distribution: "app-store",
        allowDevelopmentProvisioning: true,
        identitySha1: developmentSha1,
        deviceUdid: localUdid,
      },
      new Date("2026-07-10T00:00:00Z"),
    );
    expect(reason).toContain("expired");
  });

  it("rejects bundle id mismatches before anything else", () => {
    const reason = profileRejectionReason(developmentProfile("other.bundle"), {
      bundleIdentifier: "app.voyavpn.desktop",
      distribution: "app-store",
      allowDevelopmentProvisioning: true,
      identitySha1: developmentSha1,
      deviceUdid: localUdid,
    });
    expect(reason).toContain("bundle id");
  });
});

describe("selectProvisioningProfile", () => {
  const criteria = {
    bundleIdentifier: "app.voyavpn.desktop",
    distribution: "app-store",
    allowDevelopmentProvisioning: true,
    identitySha1: developmentSha1,
    deviceUdid: localUdid,
  };

  it("selects the development profile for a development identity regardless of directory order", () => {
    for (const profiles of [sixProfileDirectory(), sixProfileDirectory().reverse()]) {
      const { profile } = selectProvisioningProfile(profiles, criteria);
      expect(profile?.name).toBe("app.voyavpn.desktop development");
    }
  });

  it("rejects distribution profiles because the signing certificate is not embedded", () => {
    const { profile, rejections } = selectProvisioningProfile([appStoreProfile("app.voyavpn.desktop")], criteria);
    expect(profile).toBeNull();
    expect(rejections[0].reason).toContain("is not in the profile's DeveloperCertificates");
  });

  it("rejects development profiles that do not provision this Mac", () => {
    const foreignDevice = {
      ...developmentProfile("app.voyavpn.desktop"),
      provisionedDevices: ["00000000-0000000000000000"],
    };
    const { profile, rejections } = selectProvisioningProfile([foreignDevice], criteria);
    expect(profile).toBeNull();
    expect(rejections[0].reason).toContain("ProvisionedDevices");
  });

  it("selects the Developer ID profile for a Developer ID identity", () => {
    const { profile } = selectProvisioningProfile(sixProfileDirectory(), {
      bundleIdentifier: "app.voyavpn.desktop.PacketTunnel",
      distribution: "developer-id",
      identitySha1: developerIdSha1,
      deviceUdid: localUdid,
    });
    expect(profile?.name).toBe("app.voyavpn.desktop.PacketTunnel developer id");
  });

  it("skips the certificate check when no identity is provided", () => {
    const { profile } = selectProvisioningProfile(sixProfileDirectory(), {
      bundleIdentifier: "app.voyavpn.desktop",
      distribution: "developer-id",
      identitySha1: null,
      deviceUdid: null,
    });
    expect(profile?.name).toBe("app.voyavpn.desktop developer id");
  });
});

describe("formatProfileSelectionError", () => {
  it("lists every rejection and points at the runbook", () => {
    const { rejections } = selectProvisioningProfile(sixProfileDirectory(), {
      bundleIdentifier: "app.voyavpn.desktop",
      distribution: "app-store",
      allowDevelopmentProvisioning: true,
      identitySha1: "4444444444444444444444444444444444444444",
      deviceUdid: localUdid,
    });
    const message = formatProfileSelectionError("macOS app", "app.voyavpn.desktop", rejections, "/certs");
    expect(message).toContain("app.voyavpn.desktop app store");
    expect(message).toContain("docs/release/macos-local-tun-testing.md");
    expect(message).toContain("/certs");
  });

  it("mentions when no profiles exist at all", () => {
    const message = formatProfileSelectionError("macOS app", "app.voyavpn.desktop", [], "/certs");
    expect(message).toContain("No .provisionprofile/.mobileprovision files were found");
  });
});

describe("shared provisioning capability check", () => {
  function profile(networkExtensions, appGroups = ["group.app.voyavpn.desktop"]) {
    return { appGroups, networkExtensions, path: "/tmp/VoyaVPN.provisionprofile" };
  }

  it("accepts the distribution-specific NetworkExtension value", () => {
    expect(() =>
      assertProfileCapabilities(profile(["packet-tunnel-provider-systemextension"]), {
        label: "PacketTunnel",
        distribution: "developer-id",
      }),
    ).not.toThrow();
    expect(() =>
      assertProfileCapabilities(profile(["packet-tunnel-provider"]), {
        label: "PacketTunnel",
        distribution: "app-store",
      }),
    ).not.toThrow();
  });

  // sign-app.mjs used to skip the distribution-specific check whenever the
  // profile merely contained the generic value, so a Developer ID app was
  // signed without `-systemextension` and shipped mis-entitled.
  it("rejects a Developer ID profile that only carries the generic value", () => {
    expect(() =>
      assertProfileCapabilities(profile(["packet-tunnel-provider"]), {
        label: "App",
        distribution: "developer-id",
      }),
    ).toThrow(/does not include packet-tunnel-provider-systemextension/u);
  });

  it("rejects a profile without the shared app group", () => {
    expect(() =>
      assertProfileCapabilities(profile(["packet-tunnel-provider"], []), {
        label: "App",
        distribution: "app-store",
      }),
    ).toThrow(/does not include group\.app\.voyavpn\.desktop/u);
  });
});

describe("renderProfileEntitlements", () => {
  const profile = () =>
    profileFixture({ keychainAccessGroups: [], appSandbox: false, networkClient: true, networkServer: false });

  it("forwards user-selected file access from the base plist, which no profile lists", () => {
    const withAccess = renderProfileEntitlements(profile(), { appSandbox: true, userSelectedReadWrite: true });
    expect(withAccess).toContain("<key>com.apple.security.files.user-selected.read-write</key>\n  <true/>");
    expect(withAccess).toContain("<key>com.apple.security.app-sandbox</key>\n  <true/>");

    // The PacketTunnel base plist has no save panel to serve.
    const without = renderProfileEntitlements(profile(), { appSandbox: true });
    expect(without).not.toContain("files.user-selected");
  });

  it("derives identifiers and defaults from the profile", () => {
    const xml = renderProfileEntitlements(profile(), {});
    expect(xml).toContain("<string>4LUKJ56532.app.voyavpn.desktop</string>");
    expect(xml).toContain("<string>4LUKJ56532.*</string>");
    expect(xml).toContain("<string>packet-tunnel-provider</string>");
    expect(xml).toContain("com.apple.security.network.client");
    expect(xml).not.toContain("com.apple.security.network.server");
    expect(xml).not.toContain("com.apple.security.app-sandbox");
  });
});

describe("store submission entitlements", () => {
  // The shape of a real Mac App Store profile: every NetworkExtension type the
  // App ID enables, plus team wildcards for app and keychain groups.
  const allNetworkExtensions = [
    "app-proxy-provider",
    "content-filter-provider",
    "packet-tunnel-provider",
    "dns-proxy",
    "dns-settings",
    "relay",
    "url-filter-provider",
    "hotspot-provider",
  ];
  const storeProfile = () => ({
    ...appStoreProfile("app.voyavpn.desktop"),
    networkExtensions: allNetworkExtensions,
    appGroups: ["group.app.voyavpn.desktop", "4LUKJ56532.*"],
    keychainAccessGroups: ["4LUKJ56532.*"],
  });

  it("tells store distribution profiles from development and Developer ID ones", () => {
    expect(isStoreDistributionProfile(storeProfile())).toBe(true);
    expect(isStoreDistributionProfile(developmentProfile("app.voyavpn.desktop"))).toBe(false);
    expect(isStoreDistributionProfile(developerIdProfile("app.voyavpn.desktop"))).toBe(false);
  });

  it("signs a store build with only the concrete values the app uses", () => {
    expect(signedNetworkExtensions(storeProfile())).toEqual(["packet-tunnel-provider"]);

    const xml = renderProfileEntitlements(storeProfile(), { appSandbox: true, userSelectedReadWrite: true });
    expect(xml).toContain("<string>group.app.voyavpn.desktop</string>");
    expect(xml).not.toContain("4LUKJ56532.*");
    expect(xml).not.toContain("keychain-access-groups");
    expect(xml).not.toContain("app-proxy-provider");
    expect(xml).toContain("<string>packet-tunnel-provider</string>");
  });

  it("keeps the profile's full grant for development signing", () => {
    const development = {
      ...developmentProfile("app.voyavpn.desktop"),
      networkExtensions: allNetworkExtensions,
      appGroups: ["group.app.voyavpn.desktop", "4LUKJ56532.*"],
      keychainAccessGroups: ["4LUKJ56532.*"],
    };
    expect(signedNetworkExtensions(development)).toEqual(allNetworkExtensions);
    const xml = renderProfileEntitlements(development, { appSandbox: true });
    expect(xml).toContain("4LUKJ56532.*");
    expect(xml).toContain("keychain-access-groups");
  });
});

describe("embedProvisioningProfile", () => {
  const roots = [];
  afterEach(() => {
    for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
  });

  function scratch() {
    const root = mkdtempSync(join(tmpdir(), "voya-profile-"));
    roots.push(root);
    const source = join(root, "downloaded.provisionprofile");
    writeFileSync(source, Buffer.from([0x30, 0x82, 0x01, 0x02]));
    return { root, source };
  }

  it("copies the profile bytes into a folder that does not exist yet", () => {
    const { root, source } = scratch();
    const destination = join(root, "VoyaPacketTunnel.appex", "Contents", "embedded.provisionprofile");

    embedProvisioningProfile(source, destination);

    expect(readFileSync(destination)).toEqual(readFileSync(source));
  });

  // Browser-downloaded profiles are quarantined; build 352 embedded them with
  // the attribute and App Store Connect rejected the package (ITMS-91109).
  it.runIf(process.platform === "darwin")("leaves a downloaded profile's quarantine behind", () => {
    const { root, source } = scratch();
    const quarantine = "0281;6a46dacf;;D5853559-011B-43DD-B861-15287EBF3D57";
    expect(capture("xattr", ["-w", "com.apple.quarantine", quarantine, source]).status).toBe(0);
    const destination = join(root, "embedded.provisionprofile");

    embedProvisioningProfile(source, destination);

    expect(capture("xattr", ["-p", "com.apple.quarantine", source]).stdout.trim()).toBe(quarantine);
    expect(capture("xattr", ["-p", "com.apple.quarantine", destination]).status).not.toBe(0);
  });
});

describe("validateProvisioningProfile", () => {
  const criteria = { distribution: "app-store", identitySha1: distributionSha1, deviceUdid: localUdid };

  it("accepts a profile that matches the criteria, its team and the capabilities", () => {
    expect(() =>
      validateProvisioningProfile(appStoreProfile("app.voyavpn.desktop"), "macOS app", "app.voyavpn.desktop", criteria),
    ).not.toThrow();
  });

  it("reports the selection rejection reason first", () => {
    expect(() =>
      validateProvisioningProfile(
        appStoreProfile("app.voyavpn.desktop"),
        "PacketTunnel",
        "app.voyavpn.desktop.PacketTunnel",
        criteria,
      ),
    ).toThrow(/PacketTunnel provisioning profile .* cannot be used: bundle id is app\.voyavpn\.desktop, expected/u);
  });

  // sign-app.mjs used to skip this check, so a profile whose App ID prefix
  // belonged to another team signed a bundle that then failed to launch.
  it("rejects an application identifier outside the profile's own team", () => {
    const profile = appStoreProfile("app.voyavpn.desktop");
    profile.applicationIdentifier = "OTHERTEAM1.app.voyavpn.desktop";

    expect(() => validateProvisioningProfile(profile, "macOS app", "app.voyavpn.desktop", criteria)).toThrow(
      /application identifier does not match its team identifier/u,
    );
  });

  it("still runs the shared capability check", () => {
    const profile = appStoreProfile("app.voyavpn.desktop");
    profile.appGroups = [];

    expect(() => validateProvisioningProfile(profile, "macOS app", "app.voyavpn.desktop", criteria)).toThrow(
      /does not include group\.app\.voyavpn\.desktop/u,
    );
  });
});

describe("inferDistribution", () => {
  const appContents = "/bundle/VoyaVPN.app/Contents";
  const appProfile = join(appContents, "embedded.provisionprofile");
  const systemExtension = join(appContents, "Library", "SystemExtensions", "app.voyavpn.desktop.PacketTunnel.systemextension");

  it("honours an explicit VOYAVPN_MACOS_DISTRIBUTION without reading the bundle", () => {
    const decodeProfile = () => {
      throw new Error("should not decode");
    };
    expect(inferDistribution({ appContents, env: { VOYAVPN_MACOS_DISTRIBUTION: "dmg" }, exists: () => true, decodeProfile }))
      .toBe("developer-id");
    expect(inferDistribution({ appContents, env: { VOYAVPN_MACOS_DISTRIBUTION: "mas" }, exists: () => true, decodeProfile }))
      .toBe("app-store");
  });

  // The embedded profile is the authority: it exists before signing (so it
  // works where the entitlement check create-dmg used to run could not) and
  // names both the certificate lane and the NetworkExtension flavour.
  it("reads the distribution from the embedded app profile when there is one", () => {
    const decoded = new Map([
      ["developer-id", developerIdProfile("app.voyavpn.desktop")],
      ["app-store", appStoreProfile("app.voyavpn.desktop")],
      ["systemextension-only", profileFixture({ networkExtensions: ["packet-tunnel-provider-systemextension"] })],
    ]);
    for (const [expected, profile] of [
      ["developer-id", decoded.get("developer-id")],
      ["app-store", decoded.get("app-store")],
      ["developer-id", decoded.get("systemextension-only")],
    ]) {
      expect(
        inferDistribution({
          appContents,
          env: {},
          exists: (path) => path === appProfile,
          decodeProfile: (path) => {
            expect(path).toBe(appProfile);
            return profile;
          },
        }),
      ).toBe(expected);
    }
  });

  it("falls back to the staged bundle shape, and to the app-extension lane", () => {
    const decodeProfile = () => {
      throw new Error("no profile to decode");
    };
    expect(inferDistribution({ appContents, env: {}, exists: (path) => path === systemExtension, decodeProfile }))
      .toBe("developer-id");
    expect(inferDistribution({ appContents, env: {}, exists: () => false, decodeProfile })).toBe("app-store");
  });
});
