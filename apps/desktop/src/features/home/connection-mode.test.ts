import { describe, expect, it } from "vitest";

import type { SysProxyChanged, TunChanged } from "@/ipc/bindings";
import {
  CONNECTION_MODE_OPTIONS,
  deriveConnectionMode,
  isPacActive,
} from "./connection-mode";

function sysProxy(requestedMode: SysProxyChanged["requestedMode"]): SysProxyChanged {
  return { requestedMode, effectiveMode: requestedMode, pacAvailable: true, proxy: null };
}

function tun(enabled: boolean): TunChanged {
  return {
    enabled,
    backend: "process",
    providerState: "notApplicable",
    nativeComponentReady: false,
    lastProviderError: null,
  };
}

describe("deriveConnectionMode", () => {
  it("prefers vpn whenever TUN is enabled, regardless of sysproxy mode", () => {
    expect(deriveConnectionMode(sysProxy("forcedClear"), tun(true))).toBe("vpn");
    expect(deriveConnectionMode(sysProxy("pac"), tun(true))).toBe("vpn");
    expect(deriveConnectionMode(null, tun(true))).toBe("vpn");
  });

  it("maps forcedChange and pac to systemProxy", () => {
    expect(deriveConnectionMode(sysProxy("forcedChange"), tun(false))).toBe("systemProxy");
    expect(deriveConnectionMode(sysProxy("pac"), null)).toBe("systemProxy");
  });

  it("treats forcedClear, unchanged, and missing state as proxyOnly", () => {
    expect(deriveConnectionMode(sysProxy("forcedClear"), tun(false))).toBe("proxyOnly");
    expect(deriveConnectionMode(sysProxy("unchanged"), null)).toBe("proxyOnly");
    expect(deriveConnectionMode(null, null)).toBe("proxyOnly");
  });
});

describe("isPacActive", () => {
  it("is true only for the pac requested mode", () => {
    expect(isPacActive(sysProxy("pac"))).toBe(true);
    expect(isPacActive(sysProxy("forcedChange"))).toBe(false);
    expect(isPacActive(null)).toBe(false);
  });
});

describe("CONNECTION_MODE_OPTIONS", () => {
  it("keeps the Hiddify ordering", () => {
    expect(CONNECTION_MODE_OPTIONS).toEqual(["proxyOnly", "systemProxy", "vpn"]);
  });
});
