import { spawn } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const rawArgs = process.argv.slice(2);
const writeStableConfigOnly = rawArgs.includes("--write-stable-updater-config");
const tauriArgs = rawArgs.filter((arg) => arg !== "--write-stable-updater-config");
const args = ["build", ...tauriArgs];
const env = { ...process.env };
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const localTauriBin = resolve(
  repoRoot,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "tauri.cmd" : "tauri",
);
const tauriBin = existsSync(localTauriBin) ? localTauriBin : "tauri";
const stableOverlayPath = resolve(repoRoot, "target", "release-config", "tauri.updater.stable.generated.json");

if (env.CI === "1") {
  env.CI = "true";
} else if (env.CI === "0") {
  env.CI = "false";
}

function truthy(value) {
  return /^(1|true|yes|on)$/i.test(String(value ?? "").trim());
}

function falsey(value) {
  return /^(0|false|no|off)$/i.test(String(value ?? "").trim());
}

function releaseChannel() {
  return (env.VOYAVPN_RELEASE_CHANNEL ?? env.RELEASE_CHANNEL ?? env.CHANNEL ?? "").trim().toLowerCase();
}

function requestedStableUpdaterConfig() {
  const explicit = env.VOYAVPN_TAURI_UPDATER_CONFIG;
  if (explicit !== undefined) {
    if (truthy(explicit) || String(explicit).trim().toLowerCase() === "stable") {
      return true;
    }
    if (falsey(explicit)) {
      return false;
    }
    throw new Error("VOYAVPN_TAURI_UPDATER_CONFIG must be stable, true, or false.");
  }

  return writeStableConfigOnly || releaseChannel() === "stable";
}

function firstEnv(...names) {
  for (const name of names) {
    const value = env[name];
    if (value !== undefined && value !== null && String(value).trim().length > 0) {
      return String(value).trim();
    }
  }
  return null;
}

function placeholderText(value) {
  return (
    !value ||
    /placeholder|replace_before_release|replace-before-release|changeme|\btodo\b|\btbd\b|voyavpn\.example/i.test(
      String(value),
    )
  );
}

function forbiddenStableHost(hostname) {
  const host = hostname.toLowerCase();
  return (
    host === "example.com" ||
    host.endsWith(".example.com") ||
    host.endsWith(".example") ||
    host.includes("example") ||
    host === "github.com" ||
    host.endsWith(".github.com") ||
    host === "githubusercontent.com" ||
    host.endsWith(".githubusercontent.com") ||
    host === "github.io" ||
    host.endsWith(".github.io") ||
    host === "localhost" ||
    host === "127.0.0.1" ||
    host === "::1" ||
    host.endsWith(".test") ||
    host.includes("placeholder")
  );
}

function stableUpdaterBaseUrl() {
  const value = firstEnv("VOYAVPN_UPDATES_BASE_URL");
  if (!value) {
    throw new Error("VOYAVPN_UPDATES_BASE_URL is required for stable Tauri updater builds.");
  }

  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error(`VOYAVPN_UPDATES_BASE_URL is not a valid URL: ${value}`);
  }

  if (parsed.protocol !== "https:") {
    throw new Error(`VOYAVPN_UPDATES_BASE_URL must use https for stable builds: ${value}`);
  }
  if (forbiddenStableHost(parsed.hostname)) {
    throw new Error(
      `VOYAVPN_UPDATES_BASE_URL must not use example, GitHub, placeholder, localhost, or .test hosts: ${value}`,
    );
  }

  parsed.hash = "";
  parsed.search = "";
  return parsed.toString().replace(/\/+$/g, "");
}

function stableUpdaterPublicKey() {
  const value = firstEnv("VOYAVPN_UPDATER_PUBLIC_KEY", "TAURI_UPDATER_PUBLIC_KEY");
  if (placeholderText(value) || value.length < 32) {
    throw new Error("VOYAVPN_UPDATER_PUBLIC_KEY must be the approved non-placeholder Tauri updater public key.");
  }
  return value;
}

function assertStableSigningInput() {
  if (!firstEnv("TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PATH")) {
    throw new Error(
      "TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH is required when stable updater artifacts are enabled.",
    );
  }
}

function writeStableUpdaterOverlay() {
  assertStableSigningInput();
  const baseUrl = stableUpdaterBaseUrl();
  const publicKey = stableUpdaterPublicKey();
  const overlay = {
    bundle: {
      createUpdaterArtifacts: true,
    },
    plugins: {
      updater: {
        pubkey: publicKey,
        endpoints: [`${baseUrl}/latest.json`],
        windows: {
          installMode: "passive",
        },
      },
    },
  };

  mkdirSync(dirname(stableOverlayPath), { recursive: true });
  writeFileSync(stableOverlayPath, `${JSON.stringify(overlay, null, 2)}\n`);
  return stableOverlayPath;
}

try {
  if (requestedStableUpdaterConfig()) {
    const overlayPath = writeStableUpdaterOverlay();
    console.log(`Using stable Tauri updater config overlay: ${overlayPath}`);
    if (writeStableConfigOnly) {
      process.exit(0);
    }
    args.push("--config", overlayPath);
  }
} catch (error) {
  console.error(error.message);
  process.exit(1);
}

const child = spawn(tauriBin, args, {
  env,
  shell: process.platform === "win32",
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
