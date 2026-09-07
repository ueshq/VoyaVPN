import { describe, expect, it } from "vitest";

import { parsePluginkitMatches, planPacketTunnelFix } from "./ne-doctor.mjs";
import { packetTunnelBundleIdentifier } from "./tunnel-layout.mjs";

const providerId = packetTunnelBundleIdentifier;

function appexEntry(path, overrides = {}) {
  return { type: "appex", path, appBundle: path.split(".app/")[0] + ".app", stale: true, ...overrides };
}

describe("pluginkit -mDvvv parsing", () => {
  it("extracts appex paths from a real pluginkit listing", () => {
    const output = [
      `   ${providerId}(1.0)`,
      `\tPath = /Applications/VoyaVPN.app/Contents/PlugIns/${providerId}.appex`,
      `\tUUID = 6D0F0F0F-0000-0000-0000-000000000000`,
      `+   ${providerId}(1.0) /Users/afu/Dev/VoyaVPN/target/release/bundle/macos/VoyaVPN.app/Contents/PlugIns/${providerId}.appex`,
    ].join("\n");

    expect(parsePluginkitMatches(output)).toEqual([
      `/Applications/VoyaVPN.app/Contents/PlugIns/${providerId}.appex`,
      `/Users/afu/Dev/VoyaVPN/target/release/bundle/macos/VoyaVPN.app/Contents/PlugIns/${providerId}.appex`,
    ]);
  });

  it("normalizes file:// URLs, strips trailing punctuation, and de-duplicates", () => {
    const path = `/Applications/VoyaVPN.app/Contents/PlugIns/${providerId}.appex`;
    const output = [`Registered at file://${path},`, `Also at ${path};`, `And at ${path}`].join("\n");

    expect(parsePluginkitMatches(output)).toEqual([path]);
  });

  it("keeps paths that contain spaces", () => {
    const path = `/Users/afu/My Builds/VoyaVPN.app/Contents/PlugIns/${providerId}.appex`;

    expect(parsePluginkitMatches(`Path = ${path}\n`)).toEqual([path]);
  });

  it("separates two provider paths that share one line", () => {
    const a = `/Applications/VoyaVPN.app/Contents/PlugIns/${providerId}.appex`;
    const b = `/Users/afu/builds/VoyaVPN.app/Contents/PlugIns/${providerId}.appex`;

    expect(parsePluginkitMatches(`${a} and ${b}`)).toEqual([a, b]);
  });

  it("ignores lines for other plugin identifiers", () => {
    const output = "/Applications/Other.app/Contents/PlugIns/com.example.Other.appex\n";

    expect(parsePluginkitMatches(output)).toEqual([]);
  });
});

describe("--fix preconditions", () => {
  const stale = [appexEntry(`/Applications/Old.app/Contents/PlugIns/${providerId}.appex`)];
  const providerPath = `/Applications/VoyaVPN.app/Contents/PlugIns/${providerId}.appex`;

  it("returns the stale appex entries when every precondition holds", () => {
    expect(planPacketTunnelFix(stale, { providerPath, providerExists: true })).toEqual(stale);
  });

  // A mistyped --app makes every registration "not legal" and therefore stale,
  // so the old ordering (unregister first, validate later) left the machine
  // with no PacketTunnel registration at all.
  it("refuses before unregistering anything when the selected app has no provider", () => {
    expect(() => planPacketTunnelFix(stale, { providerPath, providerExists: false })).toThrow(
      /Legal PacketTunnel provider is missing.*nothing was unregistered/su,
    );
  });

  it("refuses while VoyaVPN or the PacketTunnel provider is running", () => {
    expect(() =>
      planPacketTunnelFix(stale, {
        providerPath,
        providerExists: true,
        runningExecutables: ["VoyaVPN", "VoyaPacketTunnel"],
      }),
    ).toThrow(/VoyaVPN is still running \(VoyaVPN, VoyaPacketTunnel\)/u);
  });

  it("requires --yes before removing more than one registration", () => {
    const many = [
      appexEntry(`/Applications/A.app/Contents/PlugIns/${providerId}.appex`),
      appexEntry(`/Applications/B.app/Contents/PlugIns/${providerId}.appex`),
    ];

    expect(() => planPacketTunnelFix(many, { providerPath, providerExists: true })).toThrow(
      /would unregister 2 PacketTunnel registrations/u,
    );
    expect(planPacketTunnelFix(many, { providerPath, providerExists: true, assumeYes: true })).toEqual(many);
  });

  it("never unregisters healthy entries or system extensions", () => {
    const entries = [
      appexEntry(providerPath, { stale: false }),
      { type: "systemextension", path: "/Applications/VoyaVPN.app", stale: true },
      ...stale,
    ];

    expect(planPacketTunnelFix(entries, { providerPath, providerExists: true })).toEqual(stale);
  });
});
