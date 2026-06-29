import { describe, expect, it, vi } from "vitest";

import {
  assertManualLinksSafe,
  checkAppUpdatePaths,
  installCheckedAppUpdate,
  loadAppUpdatePaths,
  type AppUpdateFlowDeps,
} from "@/features/updates/app-update-flow";
import type { ManualAppUpdateLinks } from "@/ipc/bindings";

describe("app update flow", () => {
  it("keeps manual CDN links visible when the automatic updater check fails", async () => {
    const manualLinks = makeManualLinks();
    const deps = makeDeps({
      checkForAppUpdate: vi.fn().mockRejectedValue(new Error("updater endpoint unavailable")),
      manualAppUpdateLinks: vi.fn().mockResolvedValue(manualLinks),
    });

    const result = await checkAppUpdatePaths(false, true, null, deps);

    expect(result.updaterCheck).toBeNull();
    expect(result.updaterError).toBe("updater endpoint unavailable");
    expect(result.manualLinks).toEqual(manualLinks);
    expect(result.manualError).toBeNull();
    expect(deps.recordAppUpdateDiagnostic).toHaveBeenCalledWith(
      "check",
      "failure",
      "updater endpoint unavailable",
    );
  });

  it("maps available app updates to plain UI data and closes the updater resource", async () => {
    const close = vi.fn().mockResolvedValue(undefined);
    const deps = makeDeps({
      checkForAppUpdate: vi.fn().mockResolvedValue(
        makeTauriUpdate({
          close,
          rawJson: { downloadUrl: "https://cdn.voyavpn.test/stable/latest.json" },
        }),
      ),
    });

    const result = await checkAppUpdatePaths(false, true, null, deps);

    expect(result.updaterCheck?.update).toMatchObject({
      currentVersion: "1.0.0",
      downloadUrl: "https://cdn.voyavpn.test/stable/latest.json",
      version: "2.1.0",
    });
    expect(close).toHaveBeenCalledTimes(1);
    expect(deps.recordAppUpdateDiagnostic).toHaveBeenCalledWith("check", "success", null);
  });

  it("installs an available app update and marks restart as required", async () => {
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
    const close = vi.fn().mockResolvedValue(undefined);
    const deps = makeDeps({
      checkForAppUpdate: vi.fn().mockResolvedValue(makeTauriUpdate({ close, downloadAndInstall })),
    });

    const result = await installCheckedAppUpdate(deps);

    expect(downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(close).toHaveBeenCalledTimes(1);
    expect(result).toEqual({
      currentVersion: "1.0.0",
      installedVersion: "2.1.0",
      restartRequired: true,
      state: "installed",
    });
    expect(deps.recordAppUpdateDiagnostic).toHaveBeenCalledWith("install", "success", null);
  });

  it("returns noUpdate and records a skipped install diagnostic when no app update is available", async () => {
    const deps = makeDeps({ checkForAppUpdate: vi.fn().mockResolvedValue(null) });

    const result = await installCheckedAppUpdate(deps);

    expect(result).toEqual({
      currentVersion: "1.0.0",
      installedVersion: null,
      restartRequired: false,
      state: "noUpdate",
    });
    expect(deps.recordAppUpdateDiagnostic).toHaveBeenCalledWith("install", "skipped", null);
  });

  it("records install failures without swallowing the updater error", async () => {
    const deps = makeDeps({
      checkForAppUpdate: vi.fn().mockResolvedValue(
        makeTauriUpdate({
          downloadAndInstall: vi.fn().mockRejectedValue(new Error("signature invalid")),
        }),
      ),
    });

    await expect(installCheckedAppUpdate(deps)).rejects.toThrow("signature invalid");
    expect(deps.recordAppUpdateDiagnostic).toHaveBeenCalledWith(
      "install",
      "failure",
      "signature invalid",
    );
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

  it.each([
    "javascript:alert(1)",
    "data:text/html,<script>alert(1)</script>",
    "file:///tmp/VoyaVPN-linux-x64.AppImage",
    "http://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage",
    "https://cdn.voyavpn.test.evil.example/stable/VoyaVPN-linux-x64.AppImage",
    "https://github.com.evil.example/voyavpn/VoyaVPN-linux-x64.AppImage",
    "not a url",
    " ",
  ])("rejects unsafe manual download URL %s", (url) => {
    expect(() => assertManualLinksSafe(makeManualLinks({ downloads: [makeDownload({ url })] }))).toThrow(
      "forbidden download URL",
    );
  });

  it.each([
    "https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage",
    "https://assets.cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage",
    "https://cdn.voyavpn.dev/stable/VoyaVPN-linux-x64.AppImage",
    "https://downloads.cdn.voyavpn.dev/stable/VoyaVPN-linux-x64.AppImage",
  ])("allows approved manual download host %s", (url) => {
    const links = makeManualLinks({ downloads: [makeDownload({ url })] });

    expect(assertManualLinksSafe(links)).toBe(links);
  });
});

function makeDeps(overrides: Partial<AppUpdateFlowDeps> = {}): AppUpdateFlowDeps {
  return {
    appUpdateStatus: vi.fn().mockResolvedValue({
      currentVersion: "1.0.0",
      state: "ready",
      message: null,
    }),
    checkForAppUpdate: vi.fn().mockResolvedValue(null),
    getCurrentVersion: vi.fn().mockResolvedValue("1.0.0"),
    manualAppUpdateLinks: vi.fn().mockResolvedValue(makeManualLinks()),
    recordAppUpdateDiagnostic: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function makeTauriUpdate(overrides: Record<string, unknown> = {}) {
  return {
    available: true,
    body: null,
    close: vi.fn().mockResolvedValue(undefined),
    currentVersion: "1.0.0",
    date: null,
    download: vi.fn().mockResolvedValue(undefined),
    downloadAndInstall: vi.fn().mockResolvedValue(undefined),
    install: vi.fn().mockResolvedValue(undefined),
    rawJson: {},
    version: "2.1.0",
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
    downloads: [makeDownload()],
    ...overrides,
  };
}

function makeDownload(overrides: Partial<ManualAppUpdateLinks["downloads"][number]> = {}) {
  return {
    name: "VoyaVPN-linux-x64.AppImage",
    kind: "appimage" as const,
    version: "2.0.0",
    url: "https://cdn.voyavpn.test/stable/VoyaVPN-linux-x64.AppImage",
    sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    bytes: 10,
    ...overrides,
  };
}
