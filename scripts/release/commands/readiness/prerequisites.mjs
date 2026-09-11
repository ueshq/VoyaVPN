import { readFile, stat } from "node:fs/promises";
import {
  ALLOW_UNPINNED_SING_BOX_ENV,
  DEFAULT_SING_BOX_VERSION,
  verifyStagedSingBoxSeed,
} from "../../../core/sing-box-installer.mjs";
import { repoRoot, resolveRepoPath, isDryRun } from "./inputs.mjs";

const requiredDocs = [
  "docs/release/packaging.md",
  "docs/release/ci-secrets.md",
  "docs/release/signing-notarization.md",
  "docs/release/os-smoke-matrix.md",
  "docs/release/rollback.md",
  "docs/release/runbook.md",
  "docs/release/THIRD_PARTY_NOTICES.md",
  "docs/verification/stable-release-gate.md",
];

export async function checkRequiredDocs(reporter) {
  const missing = [];
  const empty = [];

  for (const doc of requiredDocs) {
    const path = resolveRepoPath(doc);
    try {
      const fileStat = await stat(path);
      if (!fileStat.isFile()) {
        missing.push(doc);
        continue;
      }
      const text = await readFile(path, "utf8");
      if (text.trim().length === 0) {
        empty.push(doc);
      }
    } catch (error) {
      if (error && error.code === "ENOENT") {
        missing.push(doc);
        continue;
      }
      throw error;
    }
  }

  if (missing.length > 0 || empty.length > 0) {
    reporter.fail("required release documents", [
      ...missing.map((doc) => `missing: ${doc}`),
      ...empty.map((doc) => `empty: ${doc}`),
    ]);
    return;
  }

  reporter.pass("required release documents", [`found ${requiredDocs.length} required docs`]);
}

export async function checkNotices(reporter) {
  const noticesPath = resolveRepoPath("docs/release/THIRD_PARTY_NOTICES.md");
  const text = await readFile(noticesPath, "utf8");
  const requiredTerms = ["VoyaVPN", "sing-box", "GPL"];
  const missing = requiredTerms.filter((term) => !text.includes(term));

  if (missing.length > 0) {
    reporter.fail("third-party notices coverage", [`missing term(s): ${missing.join(", ")}`]);
    return;
  }

  reporter.pass("third-party notices coverage", ["notices mention app, core scope, and core license families"]);
}

/**
 * Re-verifies the seed that `pnpm tauri:build` would bundle. The seed directory
 * is gitignored, so a stale, foreign-architecture, or locally replaced binary is
 * invisible to every other gate; this reads its manifest back and compares it to
 * the digest pinned in scripts/core/sing-box-installer.mjs.
 */
export async function checkCoreSeedPinning(reporter, { verifySeed = verifyStagedSingBoxSeed } = {}) {
  const verification = verifySeed({ repoRoot, version: DEFAULT_SING_BOX_VERSION });

  if (!verification.staged) {
    reporter.pass("bundled sing-box seed", [
      `no seed staged for ${process.platform}:${process.arch}; nothing would be bundled`,
    ]);
    return;
  }

  if (!verification.ok) {
    // A digest that disagrees means the staged bytes are not the pinned bytes,
    // which is an integrity failure in any mode. Every other reason means the
    // seed is merely stale or unverifiable and the next build re-stages it.
    const tampered =
      verification.code === "archive-digest-mismatch" || verification.code === "executable-digest-mismatch";
    const details = [
      `${DEFAULT_SING_BOX_VERSION} seed cannot be trusted: ${verification.reason}`,
      "run `pnpm core:sing-box:install --force-fetch` to re-stage a verified seed",
    ];
    if (tampered) {
      reporter.fail("bundled sing-box seed", details);
    } else {
      reporter.blocker("bundled sing-box seed", details);
    }
    return;
  }

  if (!verification.pinned) {
    reporter.blocker("bundled sing-box seed", [
      `${verification.manifest.assetName} was staged with ${ALLOW_UNPINNED_SING_BOX_ENV}; its content is unverified`,
    ]);
    return;
  }

  reporter.pass("bundled sing-box seed", [
    `${verification.manifest.assetName} matches the pinned SHA-256 ${verification.manifest.sha256}`,
  ]);
}

const signingInputNames = [
  "APPLE_CERTIFICATE",
  "APPLE_CERTIFICATE_PASSWORD",
  "APPLE_ID",
  "APPLE_PASSWORD",
  "APPLE_TEAM_ID",
  "WINDOWS_CERTIFICATE_BASE64",
  "WINDOWS_CERTIFICATE_PASSWORD",
];

/**
 * This check only needs to know that a signing input exists, so a caller that
 * would rather not hand the secret itself to this process (the release workflow)
 * can pass `HAS_<NAME>=true` instead of the value.
 */
export function hasSigningInput(name, env = process.env) {
  return Boolean(env[name]) || String(env[`HAS_${name}`] ?? "").trim().toLowerCase() === "true";
}

export async function checkStableEnvironment(reporter, options) {
  if (isDryRun(options)) {
    reporter.pass("stable-only secrets", ["dry-run mode does not require signing, notarization, or publication secrets"]);
    return;
  }

  const missing = [];

  if (!options.cdnBaseUrl && !process.env.VOYAVPN_CDN_BASE_URL) {
    missing.push("VOYAVPN_CDN_BASE_URL or --cdn-base-url");
  }
  // The updater base URL silently falls back to the CDN base URL, so stable
  // mode must require it explicitly instead of comparing the two values.
  if (!options.updatesBaseUrl && !process.env.VOYAVPN_UPDATES_BASE_URL) {
    missing.push("VOYAVPN_UPDATES_BASE_URL or --updates-base-url");
  }
  if (!hasSigningInput("TAURI_SIGNING_PRIVATE_KEY") && !hasSigningInput("TAURI_SIGNING_PRIVATE_KEY_PATH")) {
    missing.push("TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH");
  }

  for (const name of signingInputNames) {
    if (!hasSigningInput(name)) {
      missing.push(name);
    }
  }

  if (missing.length > 0) {
    reporter.fail("stable required env inputs", missing.map((name) => `missing: ${name}`));
    return;
  }

  reporter.pass("stable required env inputs", [
    "CDN/updater base URLs and signing env names are present; secret values are not printed",
  ]);
}
