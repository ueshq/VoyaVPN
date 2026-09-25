import { relative, resolve } from "node:path";

import { environmentValue, falsey, truthy } from "../lib/common.mjs";
import { sha256Text, writeJson } from "../lib/fs.mjs";
import { resolveApprovedUpdaterPublicKey } from "../release/updater-signatures.mjs";
import { normalizeReleaseUrl } from "../release/validation.mjs";

function stableUpdaterBaseUrl(env) {
  const value = environmentValue(env, "VOYAVPN_UPDATES_BASE_URL");
  if (!value) {
    throw new Error("VOYAVPN_UPDATES_BASE_URL is required for stable Tauri updater builds.");
  }

  return normalizeReleaseUrl(value, { label: "VOYAVPN_UPDATES_BASE_URL" });
}

// One resolver for the approved updater public key, shared with the staging
// verifier and the updater metadata command, so all three accept the variable
// names the release docs promise, reject a VOYAVPN/TAURI mismatch identically,
// and refuse a key that does not decode to a minisign public key. Checking only
// its length here let a malformed key be baked into a stable build, which then
// could never verify an update.
function stableUpdaterPublicKey(env) {
  return resolveApprovedUpdaterPublicKey(env);
}

export function requestedStableUpdaterConfig(env = process.env) {
  const explicit = env.VOYAVPN_TAURI_UPDATER_CONFIG;
  if (explicit !== undefined) {
    if (truthy(explicit) || String(explicit).trim().toLowerCase() === "stable") return true;
    if (falsey(explicit)) return false;
    throw new Error("VOYAVPN_TAURI_UPDATER_CONFIG must be stable, true, or false.");
  }

  return (env.VOYAVPN_RELEASE_CHANNEL ?? "").trim().toLowerCase() === "stable";
}

export function normalizeCiEnv(source = process.env) {
  const env = { ...source };
  if (env.CI === "1") env.CI = "true";
  else if (env.CI === "0") env.CI = "false";
  return env;
}

export function writeStableUpdaterOverlay({ repoRoot, env = process.env }) {
  if (!environmentValue(env, "TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PATH")) {
    throw new Error(
      "TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH is required when stable updater artifacts are enabled.",
    );
  }

  const baseUrl = stableUpdaterBaseUrl(env);
  const publicKey = stableUpdaterPublicKey(env);
  const endpoints = [`${baseUrl}/latest.json`];
  const overlay = {
    bundle: { createUpdaterArtifacts: true },
    plugins: {
      updater: {
        pubkey: publicKey,
        endpoints,
        windows: { installMode: "passive" },
      },
    },
  };
  const overlayPath = resolve(repoRoot, "target", "release-config", "tauri.updater.stable.generated.json");
  const metadataPath = resolve(
    repoRoot,
    "target",
    "release-config",
    "tauri.updater.stable.generated.metadata.json",
  );
  const overlayText = writeJson(overlayPath, overlay);
  writeJson(metadataPath, {
    path: relative(repoRoot, overlayPath).replaceAll("\\", "/"),
    sha256: sha256Text(overlayText),
    pubkeySha256: sha256Text(publicKey),
    endpoints,
    createUpdaterArtifacts: true,
  });
  return overlayPath;
}
