import { beforeEach, describe, expect, it } from "vite-plus/test";

import { makeMockSeed } from "@voya/client/mock-seed";
import type { TunStatus } from "@voya/contracts";
import { changeLocale, i18next } from "@voya/i18n";

import { tunProviderLabel } from "./tun-provider-text";

function tun(overrides: Partial<TunStatus>): TunStatus {
  return { ...makeMockSeed().tun, ...overrides };
}

describe("tunProviderLabel", () => {
  beforeEach(async () => {
    await changeLocale("en", { persist: false });
  });

  const t = () => i18next.t.bind(i18next);

  // A phone's tunnel is its own backend, not an unsupported one.
  it("names the phone backends rather than calling them unsupported", () => {
    expect(tunProviderLabel(tun({ backend: "iosPacketTunnel", providerState: "running" }), t())).toBe(
      "iOS PacketTunnel: Running",
    );
    expect(tunProviderLabel(tun({ backend: "androidVpnService", providerState: "stopped" }), t())).toBe(
      "Android VpnService: Stopped",
    );
  });

  it("appends the provider's own error when it has one", () => {
    expect(
      tunProviderLabel(
        tun({
          backend: "iosPacketTunnel",
          lastProviderError: "the tunnel did not come up within 60s",
          providerState: "error",
        }),
        t(),
      ),
    ).toBe("iOS PacketTunnel: Error: the tunnel did not come up within 60s");
  });

  it("leaves the provider's untranslated error out when asked", () => {
    const failed = tun({
      backend: "iosPacketTunnel",
      lastProviderError: "the tunnel did not come up within 60s",
      providerState: "error",
    });

    expect(tunProviderLabel(failed, t(), { includeProviderError: false })).toBe("iOS PacketTunnel: Error");
    // An explanation this app words itself is kept either way.
    expect(
      tunProviderLabel(tun({ backend: "macosPacketTunnel", providerState: "missingComponent" }), t(), {
        includeProviderError: false,
      }),
    ).toContain(i18next.t("status.macosTunnelMissing"));
  });

  it("explains a macOS extension missing from the running copy", () => {
    expect(tunProviderLabel(tun({ backend: "macosPacketTunnel", providerState: "missingComponent" }), t())).toBe(
      `macOS PacketTunnel: Missing component: ${i18next.t("status.macosTunnelMissing")}`,
    );
  });
});
