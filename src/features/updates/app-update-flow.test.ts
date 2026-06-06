import { describe, expect, it, vi } from "vitest";

import {
  assertManualLinksSafe,
  checkAppUpdatePaths,
  loadAppUpdatePaths,
  type AppUpdateFlowDeps,
} from "@/features/updates/app-update-flow";
import type { ManualAppUpdateLinks } from "@/ipc/bindings";

describe("app update flow", () => {
  it("keeps manual CDN links visible when the automatic updater check fails", async () => {
    const manualLinks = makeManualLinks();
    const deps = makeDeps({
      checkAppUpdate: vi.fn().mockRejectedValue(new Error("updater endpoint unavailable")),
      manualAppUpdateLinks: vi.fn().mockResolvedValue(manualLinks),
    });

    const result = await checkAppUpdatePaths(false, true, null, deps);

    expect(result.updaterCheck).toBeNull();
    expect(result.updaterError).toBe("updater endpoint unavailable");
    expect(result.manualLinks).toEqual(manualLinks);
    expect(result.manualError).toBeNull();
  });

  it("loads updater status and manual links independently", async () => {
    const manualLinks = makeManualLinks();
    const deps = makeDeps({
      appUpdateStatus: vi.fn().mockResolvedValue({
        currentVersion: "1.0.0",
        state: "unconfigured",
        message: "Updater does not have any endpoints set.",
      }),
      manualAppUpdateLinks: vi.fn().mockResolvedValue(manualLinks),
    });

    const result = await loadAppUpdatePaths(false, true, null, deps);

    expect(result.updaterStatus?.state).toBe("unconfigured");
    expect(result.manualLinks?.downloads[0]?.url).toBe("https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage");
  });

  it("rejects forbidden GitHub manual download URLs", () => {
    const links = makeManualLinks({
      downloads: [
        {
          name: "VoyaVPN-linux-x64.AppImage",
          kind: "appimage",
          version: "2.0.0",
          url: "https://github.com/voyavpn/voyavpn/releases/download/v2.0.0/VoyaVPN-linux-x64.AppImage",
          sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          bytes: 10,
        },
      ],
    });

    expect(() => assertManualLinksSafe(links)).toThrow("forbidden download URL");
  });
});

function makeDeps(overrides: Partial<AppUpdateFlowDeps> = {}): AppUpdateFlowDeps {
  return {
    appUpdateStatus: vi.fn().mockResolvedValue({
      currentVersion: "1.0.0",
      state: "ready",
      message: null,
    }),
    checkAppUpdate: vi.fn().mockResolvedValue({
      currentVersion: "1.0.0",
      update: null,
    }),
    installAppUpdate: vi.fn().mockResolvedValue({
      state: "noUpdate",
      currentVersion: "1.0.0",
      installedVersion: null,
    }),
    manualAppUpdateLinks: vi.fn().mockResolvedValue(makeManualLinks()),
    ...overrides,
  };
}

function makeManualLinks(overrides: Partial<ManualAppUpdateLinks> = {}): ManualAppUpdateLinks {
  return {
    currentVersion: "1.0.0",
    remoteVersion: "2.0.0",
    hasUpdate: true,
    releaseIndexUrl: "https://cdn.voyavpn.test/stable/release-index.json",
    channel: "stable",
    target: "linux",
    arch: "x64",
    downloads: [
      {
        name: "VoyaVPN-linux-x64.AppImage",
        kind: "appimage",
        version: "2.0.0",
        url: "https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage",
        sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        bytes: 10,
      },
    ],
    ...overrides,
  };
}
