import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

import { ensureSingBoxSeedForBuild } from "../core/sing-box-installer.mjs";
import { isCliEntrypoint, repoRootFromScript } from "../lib/common.mjs";
import { platformShipsCoreSeed, writeOptionalCoreSeedOverlay } from "./core-seeds.mjs";
import {
  normalizeCiEnv,
  requestedStableUpdaterConfig,
  writeStableUpdaterOverlay,
} from "./stable-updater-config.mjs";

const rustTargetPlatforms = {
  "apple-darwin": "darwin",
  "pc-windows-gnu": "win32",
  "pc-windows-msvc": "win32",
  "unknown-linux-gnu": "linux",
  "unknown-linux-musl": "linux",
};

const rustTargetArchs = {
  aarch64: "arm64",
  arm64: "arm64",
  i686: "ia32",
  x86_64: "x64",
};

/**
 * Maps a Rust target triple to the {platform, arch} pair the core seed
 * installer keys on, so a cross-target `tauri build` does not bundle the host
 * architecture's sing-box binary. `universal-apple-darwin` resolves the
 * platform only: its architecture is ambiguous and stays the host default.
 */
export function parseRustTargetTriple(triple) {
  const value = String(triple ?? "").trim();
  const match = /^([^-]+)-(.+)$/.exec(value);
  if (!match) {
    return null;
  }

  const platform = rustTargetPlatforms[match[2]];
  if (!platform) {
    return null;
  }

  const arch = rustTargetArchs[match[1]] ?? null;
  return arch ? { arch, platform } : { platform };
}

function seedTargetFromArgs(args) {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--target") {
      return parseRustTargetTriple(args[index + 1]) ?? {};
    }
    if (arg.startsWith("--target=")) {
      return parseRustTargetTriple(arg.slice("--target=".length)) ?? {};
    }
  }

  return {};
}

export async function prepareTauriInvocation(
  rawArgs,
  {
    repoRoot = repoRootFromScript(import.meta.url),
    sourceEnv = process.env,
    ensureSeed = ensureSingBoxSeedForBuild,
    writeCoreOverlay = writeOptionalCoreSeedOverlay,
    writeUpdaterOverlay = writeStableUpdaterOverlay,
    hostPlatform = process.platform,
  } = {},
) {
  const desktopRoot = resolve(repoRoot, "apps", "desktop");
  const localTauriJs = resolve(desktopRoot, "node_modules", "@tauri-apps", "cli", "tauri.js");
  const tauriArgs = rawArgs.length === 0 ? ["dev"] : [...rawArgs];
  const operation = tauriArgs[0];
  const env = operation === "build" ? normalizeCiEnv(sourceEnv) : { ...sourceEnv };

  const seedTarget = seedTargetFromArgs(tauriArgs);
  const targetPlatform = seedTarget.platform ?? hostPlatform;
  const shipsSeed = platformShipsCoreSeed(targetPlatform);

  if (operation === "build" && shipsSeed) {
    await ensureSeed({ repoRoot, ...seedTarget });
  }

  if ((operation === "dev" || operation === "build") && shipsSeed) {
    const configRoot = operation === "build" ? "release-config" : "tauri-config";
    const coreSeedOverlayPath = writeCoreOverlay(
      repoRoot,
      resolve(repoRoot, "target", configRoot, "tauri.core-seeds.generated.json"),
      { platform: targetPlatform },
    );
    if (coreSeedOverlayPath) {
      tauriArgs.splice(1, 0, "--config", coreSeedOverlayPath);
    }
  }

  if (operation === "build" && requestedStableUpdaterConfig(env)) {
    const overlayPath = writeUpdaterOverlay({ repoRoot, env });
    console.log(`Using stable Tauri updater config overlay: ${overlayPath}`);
    tauriArgs.splice(1, 0, "--config", overlayPath);
  }

  return {
    command: existsSync(localTauriJs) ? process.execPath : "tauri",
    commandArgs: existsSync(localTauriJs) ? [localTauriJs, ...tauriArgs] : tauriArgs,
    cwd: desktopRoot,
    env,
  };
}

export async function main(rawArgs = process.argv.slice(2)) {
  const invocation = await prepareTauriInvocation(rawArgs);
  const child = spawn(invocation.command, invocation.commandArgs, {
    cwd: invocation.cwd,
    env: invocation.env,
    shell: false,
    stdio: "inherit",
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code ?? 1);
  });
  child.on("error", (error) => {
    console.error(error.message);
    process.exit(1);
  });
}

if (isCliEntrypoint(import.meta.url)) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}
