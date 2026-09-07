import { describe, expect, it } from "vitest";

import {
  distributionFromIdentityName,
  incompatiblePacketTunnelBundle,
  packetTunnelLayout,
  packagingModeForDistribution,
  requiredNetworkExtensionValue,
  resolvePacketTunnelVersions,
} from "./tunnel-layout.mjs";

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
      "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
    );
  });

  it("uses app extension packaging for App Store and development lanes", () => {
    const layout = packetTunnelLayout(appContents, "app-store");

    expect(packagingModeForDistribution("app-store")).toBe("app-extension");
    expect(requiredNetworkExtensionValue("app-store")).toBe("packet-tunnel-provider");
    expect(layout.infoPackageType).toBe("XPC!");
    expect(layout.bundle).toBe(
      "/Applications/VoyaVPN.app/Contents/PlugIns/app.voyavpn.desktop.PacketTunnel.appex",
    );
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
