import { describe, expect, it, vi } from "vitest";

import { parseRustTargetTriple, prepareTauriInvocation } from "./cli.mjs";

describe("Tauri CLI", () => {
  it("defaults to dev and adds the generated core overlay", async () => {
    const invocation = await prepareTauriInvocation([], {
      repoRoot: "/repo",
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

  it("stages the seed for the requested cross-compilation target", async () => {
    const ensureSeed = vi.fn();
    await prepareTauriInvocation(["build", "--target", "aarch64-apple-darwin", "--bundles", "app"], {
      repoRoot: "/repo",
      sourceEnv: {},
      ensureSeed,
      writeCoreOverlay: () => null,
    });

    expect(ensureSeed).toHaveBeenCalledWith({ arch: "arm64", platform: "darwin", repoRoot: "/repo" });

    const inlineEnsureSeed = vi.fn();
    await prepareTauriInvocation(["build", "--target=x86_64-pc-windows-msvc"], {
      repoRoot: "/repo",
      sourceEnv: {},
      ensureSeed: inlineEnsureSeed,
      writeCoreOverlay: () => null,
    });

    expect(inlineEnsureSeed).toHaveBeenCalledWith({ arch: "x64", platform: "win32", repoRoot: "/repo" });
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
      sourceEnv: {},
      ensureSeed,
      writeCoreOverlay: vi.fn(),
    });

    expect(ensureSeed).not.toHaveBeenCalled();
    expect(invocation.commandArgs).toEqual(["signer", "generate"]);
  });
});
