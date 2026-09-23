import { forbiddenHostReason, placeholderText } from "../validation.mjs";
import { hasSigningInput } from "./readiness/prerequisites.mjs";
import { resolveApprovedUpdaterPublicKey } from "../updater-signatures.mjs";

const signingInputNames = [
  "APPLE_CERTIFICATE",
  "APPLE_CERTIFICATE_PASSWORD",
  "APPLE_ID",
  "APPLE_PASSWORD",
  "APPLE_TEAM_ID",
  "WINDOWS_CERTIFICATE_BASE64",
  "WINDOWS_CERTIFICATE_PASSWORD",
];

function printHelp() {
  console.log(`Usage: pnpm release -- check-hosts

Validates the stable release host and signing environment the release workflow
exports before packaging. Reuses the shared validators in scripts/release/
(forbidden hosts, placeholders, signing-input presence) instead of a
workflow-inline copy.

Reads from the environment:
  VOYAVPN_CDN_BASE_URL, VOYAVPN_UPDATES_BASE_URL   approved HTTPS CDN hosts
  VOYAVPN_UPDATER_PUBLIC_KEY                       approved Tauri updater key
  VOYAVPN_CORE_ASSETS_JSON                         core asset source document
  TAURI_SIGNING_PRIVATE_KEY[_PATH]                 updater signing material
  HAS_* presence booleans                          accepted in place of secrets

Exits non-zero when any required value is missing, a placeholder, or hosted on
a forbidden (example/GitHub/local) host.`);
}

function requireValue(name, env) {
  const value = String(env[name] ?? "").trim();
  if (!value) {
    throw new Error(`${name} is required for stable releases.`);
  }
  if (placeholderText(value)) {
    throw new Error(`${name} must not be a placeholder.`);
  }
  return value;
}

function isPresent(name, env) {
  return String(env[`HAS_${name}`] ?? "").trim().toLowerCase() === "true";
}

function requirePresent(name, env) {
  if (!hasSigningInput(name, env)) {
    throw new Error(`${name} is required for stable releases.`);
  }
}

function requireOneOfPresent(env, ...names) {
  if (!names.some((name) => hasSigningInput(name, env))) {
    throw new Error(`${names.join(" or ")} is required for stable releases.`);
  }
}

function requireHttpsUrl(name, env) {
  const value = requireValue(name, env);
  let parsed;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error(`${name} must be a valid URL.`);
  }
  if (parsed.protocol !== "https:") {
    throw new Error(`${name} must use https.`);
  }
  const reason = forbiddenHostReason(parsed.hostname);
  if (reason) {
    throw new Error(`${name} must use an approved non-placeholder CDN host (${reason}).`);
  }
}

function main(argv = [], { env = process.env } = {}) {
  if (argv.includes("--help") || argv.includes("-h")) {
    printHelp();
    return;
  }
  if (argv.length > 0) {
    throw new Error(`Unknown argument: ${argv[0]}`);
  }

  requireHttpsUrl("VOYAVPN_CDN_BASE_URL", env);
  requireHttpsUrl("VOYAVPN_UPDATES_BASE_URL", env);
  requireValue("VOYAVPN_UPDATER_PUBLIC_KEY", env);
  // The same resolver the overlay generator and the staging verifier use, so a
  // key that cannot decode fails here rather than hours later in the build.
  resolveApprovedUpdaterPublicKey(env);

  const coreAssets = JSON.parse(requireValue("VOYAVPN_CORE_ASSETS_JSON", env));
  if (!Array.isArray(coreAssets.assets) || coreAssets.assets.length === 0) {
    throw new Error("VOYAVPN_CORE_ASSETS_JSON must contain a non-empty assets array.");
  }

  requireOneOfPresent(env, "TAURI_SIGNING_PRIVATE_KEY", "TAURI_SIGNING_PRIVATE_KEY_PATH");
  for (const name of signingInputNames) {
    requirePresent(name, env);
  }
}

export { main, printHelp, isPresent };
