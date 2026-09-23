import { describe, expect, it, vi } from "vitest";

import { parseRustTargetTriple, prepareTauriInvocation } from "./cli.mjs";

describe("Tauri CLI", () => {
  it("defaults to dev and adds the generated core overlay", async () => {
    const invocation = await prepareTauriInvocation([], {
      repoRoot: "/repo",
      hostPlatform: "linux",
      sourceEnv: {},
      ensureSeed: vi.fn(),
      writeCoreOverlay: () => "/repo/target/tauri-config/tauri.core-seeds.generated.json",
    });

    expect(invocation.commandArgs).toEqual([
      "dev",
      "--config",
      "/repo/target/tauri-config/tauri.core-seeds.generated.json",
    ]);
  });

  it("prepares build seeds and normalizes CI", async () => {
    const ensureSeed = vi.fn();
    const invocation = await prepareTauriInvocation(["build", "--debug"], {
      repoRoot: "/repo",
      hostPlatform: "linux",
      sourceEnv: { CI: "1", VOYAVPN_TAURI_UPDATER_CONFIG: "false" },
      ensureSeed,
      writeCoreOverlay: () => null,
    });

    expect(ensureSeed).toHaveBeenCalledWith({ repoRoot: "/repo" });
    expect(invocation.commandArgs).toEqual(["build", "--debug"]);
    expect(invocation.env.CI).toBe("true");
  });

  it("adds stable updater and core overlays to build without changing signer passthrough", async () => {
    const sourceEnv = { CI: "0", VOYAVPN_RELEASE_CHANNEL: "stable" };
    const writeUpdaterOverlay = vi.fn(() => "/repo/target/release-config/tauri.updater.json");
    const invocation = await prepareTauriInvocation(["build", "--bundles", "app"], {
      repoRoot: "/repo",
      hostPlatform: "linux",
      sourceEnv,
      ensureSeed: vi.fn(),
      writeCoreOverlay: () => "/repo/target/release-config/tauri.core.json",
      writeUpdaterOverlay,
    });

    expect(writeUpdaterOverlay).toHaveBeenCalledWith({
      repoRoot: "/repo",
      env: expect.objectContaining({ CI: "false" }),
    });
    expect(invocation.commandArgs).toEqual([
      "build",
      "--config",
      "/repo/target/release-config/tauri.updater.json",
      "--config",
      "/repo/target/release-config/tauri.core.json",
      "--bundles",
      "app",
    ]);
  });

  it("adds the Mac App Store overlay and feature right after build", async () => {
    const writeAppStoreOverlay = vi.fn(() => ({
      overlayPath: "/repo/target/release-config/tauri.mac-app-store.generated.json",
      buildNumber: "412",
    }));
    const invocation = await prepareTauriInvocation(["build", "--bundles", "app", "--", "--locked"], {
      repoRoot: "/repo",
      hostPlatform: "darwin",
      sourceEnv: { VOYAVPN_MAC_APP_STORE: "1" },
      ensureSeed: vi.fn(),
      writeCoreOverlay: () => "/repo/target/release-config/tauri.core.json",
      writeAppStoreOverlay,
    });

    expect(writeAppStoreOverlay).toHaveBeenCalledWith({
      repoRoot: "/repo",
      env: expect.objectContaining({ VOYAVPN_MAC_APP_STORE: "1" }),
    });
    expect(invocation.commandArgs).toEqual([
      "build",
      "--config",
      "/repo/target/release-config/tauri.mac-app-store.generated.json",
      "--features",
      "mac-app-store",
      "--config",
      "/repo/target/release-config/tauri.core.json",
      "--bundles",
      "app",
      "--",
      "--locked",
    ]);
  });

  it("refuses a Mac App Store build that also asks for the stable updater", async () => {
    const writeUpdaterOverlay = vi.fn();
    const writeAppStoreOverlay = vi.fn();

    await expect(
      prepareTauriInvocation(["build"], {
        repoRoot: "/repo",
        hostPlatform: "darwin",
        sourceEnv: { VOYAVPN_MAC_APP_STORE: "true", VOYAVPN_RELEASE_CHANNEL: "stable" },
        ensureSeed: vi.fn(),
        writeCoreOverlay: () => null,
        writeUpdaterOverlay,
        writeAppStoreOverlay,
      }),
    ).rejects.toThrow(/cannot enable the stable updater/);
    expect(writeUpdaterOverlay).not.toHaveBeenCalled();
    expect(writeAppStoreOverlay).not.toHaveBeenCalled();
  });

  it("ignores the Mac App Store switch outside build", async () => {
    const writeAppStoreOverlay = vi.fn();
    const invocation = await prepareTauriInvocation(["dev"], {
      repoRoot: "/repo",
      hostPlatform: "darwin",
      sourceEnv: { VOYAVPN_MAC_APP_STORE: "1" },
      ensureSeed: vi.fn(),
      writeCoreOverlay: () => null,
      writeAppStoreOverlay,
    });

    expect(writeAppStoreOverlay).not.toHaveBeenCalled();
    expect(invocation.commandArgs).toEqual(["dev"]);
  });

  it("stages the seed for the requested cross-compilation target", async () => {
    const ensureSeed = vi.fn();
    await prepareTauriInvocation(["build", "--target", "aarch64-unknown-linux-gnu", "--bundles", "app"], {
      repoRoot: "/repo",
      hostPlatform: "linux",
      sourceEnv: {},
      ensureSeed,
      writeCoreOverlay: () => null,
    });

    expect(ensureSeed).toHaveBeenCalledWith({ arch: "arm64", platform: "linux", repoRoot: "/repo" });

    const inlineEnsureSeed = vi.fn();
    await prepareTauriInvocation(["build", "--target=x86_64-pc-windows-msvc"], {
      repoRoot: "/repo",
      hostPlatform: "linux",
      sourceEnv: {},
      ensureSeed: inlineEnsureSeed,
      writeCoreOverlay: () => null,
    });

    expect(inlineEnsureSeed).toHaveBeenCalledWith({ arch: "x64", platform: "win32", repoRoot: "/repo" });
  });

  it("stages and bundles the seed for macOS packages", async () => {
    const ensureSeed = vi.fn();
    const writeCoreOverlay = vi.fn(() => "/repo/target/release-config/tauri.core.json");
    const targeted = await prepareTauriInvocation(["build", "--target", "aarch64-apple-darwin"], {
      repoRoot: "/repo",
      hostPlatform: "linux",
      sourceEnv: {},
      ensureSeed,
      writeCoreOverlay,
    });

    expect(ensureSeed).toHaveBeenCalledWith({ arch: "arm64", platform: "darwin", repoRoot: "/repo" });
    expect(writeCoreOverlay).toHaveBeenCalledWith(
      "/repo",
      "/repo/target/release-config/tauri.core-seeds.generated.json",
      { platform: "darwin" },
    );
    expect(targeted.commandArgs).toEqual([
      "build",
      "--config",
      "/repo/target/release-config/tauri.core.json",
      "--target",
      "aarch64-apple-darwin",
    ]);
  });

  it("maps Rust target triples to core seed platforms", () => {
    expect(parseRustTargetTriple("x86_64-unknown-linux-gnu")).toEqual({ arch: "x64", platform: "linux" });
    expect(parseRustTargetTriple("aarch64-unknown-linux-musl")).toEqual({ arch: "arm64", platform: "linux" });
    expect(parseRustTargetTriple("universal-apple-darwin")).toEqual({ platform: "darwin" });
    expect(parseRustTargetTriple("x86_64-unknown-freebsd")).toBeNull();
    expect(parseRustTargetTriple(undefined)).toBeNull();
  });

  it("passes non-build commands through without build preparation", async () => {
    const ensureSeed = vi.fn();
    const invocation = await prepareTauriInvocation(["signer", "generate"], {
      repoRoot: "/repo",
      hostPlatform: "linux",
      sourceEnv: {},
      ensureSeed,
      writeCoreOverlay: vi.fn(),
    });

    expect(ensureSeed).not.toHaveBeenCalled();
    expect(invocation.commandArgs).toEqual(["signer", "generate"]);
  });
});
