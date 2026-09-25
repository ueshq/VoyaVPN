import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { truthy } from "../lib/common.mjs";
import { sha256Text, writeJson } from "../lib/fs.mjs";
import { download } from "./download.mjs";
import { coreSeedsDir, ensureSingBoxSeedForBuild } from "./sing-box-installer.mjs";

const RULE_SET_REPO = "2dust/sing-box-rules";
const RULE_SET_SEED_DIR = "rule_sets";
const RULE_SET_SEED_MANIFEST = "rule-sets.seed.json";
const SRS_MAGIC = "SRS";

/**
 * The rule sets the default routing profile names, bundled so a fresh install
 * routes China and LAN destinations directly before anything is downloaded.
 * Without them the generated config points at remote rule sets that sing-box
 * fetches through the proxy on first start.
 *
 * Upstream publishes these on two branches and has no release tags, so each
 * file is pinned by commit and SHA-256. Bump them together, as documented in
 * docs/release/sing-box-seed-pinning.md. Tags must match what
 * `voya_core::routing_seed` generates (`geosite:cn` -> `geosite-cn`).
 */
export const RULE_SET_PINS = [
  {
    tag: "geosite-cn",
    commit: "cf83f68f454487408ddd6820b5bf154ad96686c2",
    sha256: "0120caae00148e23d980cac9f93499de6d849b43a3dad11259dd2697eac05d69",
  },
  {
    tag: "geosite-private",
    commit: "cf83f68f454487408ddd6820b5bf154ad96686c2",
    sha256: "c5e69590f0a73418b6240e38c3e8b43bddc5cbffae964998c0e0a276c9b41dce",
  },
  {
    tag: "geoip-cn",
    commit: "bd658cfebfa55a34816aa993cb33b121a9f94ed4",
    sha256: "2dd53f385fd84bb0b3fc980674893cbc1d46b979bb398ba2d7a102dde8026e58",
  },
];

export function ruleSetSeedDir(repoRoot) {
  return join(coreSeedsDir(repoRoot), RULE_SET_SEED_DIR);
}

export function ruleSetUrl(pin) {
  return `https://raw.githubusercontent.com/${RULE_SET_REPO}/${pin.commit}/${pin.tag}.srs`;
}

/** Whether every pinned rule set is staged with its pinned bytes. */
export function verifyStagedRuleSets({ repoRoot, pins = RULE_SET_PINS }) {
  const dir = ruleSetSeedDir(repoRoot);
  for (const pin of pins) {
    const file = join(dir, `${pin.tag}.srs`);
    if (!existsSync(file)) {
      return { code: "missing", ok: false, reason: `${pin.tag}.srs is not staged` };
    }
    if (sha256Text(readFileSync(file)) !== pin.sha256) {
      return { code: "digest-mismatch", ok: false, reason: `${pin.tag}.srs does not match its pinned SHA-256` };
    }
  }
  return { code: "verified", ok: true, reason: null };
}

/** Whether `dir` holds any staged rule set, which is what the bundle needs. */
export function hasStagedRuleSets(dir) {
  return existsSync(dir)
    && readdirSync(dir, { withFileTypes: true }).some((entry) => entry.isFile() && entry.name.endsWith(".srs"));
}

function assertPinnedRuleSet(pin, buffer, url) {
  if (buffer.subarray(0, SRS_MAGIC.length).toString("latin1") !== SRS_MAGIC) {
    throw new Error(`${url} is not a sing-box binary rule set`);
  }
  const digest = sha256Text(buffer);
  if (digest !== pin.sha256) {
    throw new Error(`SHA-256 mismatch for ${url}: expected ${pin.sha256}, got ${digest}`);
  }
}

export async function fetchAndStageRuleSets({
  fetchImpl = fetch,
  logger = console,
  pins = RULE_SET_PINS,
  repoRoot,
}) {
  const dir = ruleSetSeedDir(repoRoot);
  mkdirSync(dir, { recursive: true });
  // Download and check everything before replacing anything, so a failed
  // fetch never leaves a mix of old and new files behind.
  const downloads = [];
  for (const pin of pins) {
    const url = ruleSetUrl(pin);
    const buffer = await download(url, { "User-Agent": "voyavpn-rule-set-installer" }, { fetchImpl });
    assertPinnedRuleSet(pin, buffer, url);
    downloads.push({ buffer, pin, url });
  }
  for (const { buffer, pin } of downloads) {
    const target = join(dir, `${pin.tag}.srs`);
    writeFileSync(`${target}.tmp`, buffer);
    renameSync(`${target}.tmp`, target);
  }
  const manifest = {
    source: `https://github.com/${RULE_SET_REPO}`,
    files: downloads.map(({ buffer, pin, url }) => ({
      bytes: buffer.length,
      commit: pin.commit,
      file: `${pin.tag}.srs`,
      sha256: pin.sha256,
      url,
    })),
  };
  writeJson(join(dir, RULE_SET_SEED_MANIFEST), manifest);
  logger.log(`  ✓ staged ${downloads.length} rule sets -> ${dir}`);
  return { dir, files: manifest.files };
}

/** Stages the pinned rule sets for a package build unless they already are. */
export async function ensureRuleSetSeedsForBuild({
  logger = console,
  repoRoot,
  stage = fetchAndStageRuleSets,
}) {
  const verification = verifyStagedRuleSets({ repoRoot });
  if (verification.ok) {
    return { dir: ruleSetSeedDir(repoRoot), status: "already-staged" };
  }
  logger.log(`- staging rule-set seeds: ${verification.reason}`);
  const result = await stage({ logger, repoRoot });
  return { ...result, status: "staged" };
}

/** Everything a package bundles from `resources/core-seeds`. */
export async function ensureCoreSeedsForBuild(options) {
  const singBox = await ensureSingBoxSeedForBuild(options);
  const ruleSets = await ensureRuleSetSeedsForBuild({ logger: options.logger, repoRoot: options.repoRoot });
  return { ...singBox, ruleSets };
}

export function shouldSkipRuleSetInstall({ env = process.env, postinstall = false } = {}) {
  if (truthy(env.VOYAVPN_SKIP_RULE_SETS_POSTINSTALL)) {
    return { reason: "VOYAVPN_SKIP_RULE_SETS_POSTINSTALL=1", skip: true };
  }
  if (postinstall && truthy(env.CI) && !truthy(env.VOYAVPN_FETCH_RULE_SETS_ON_INSTALL)) {
    return { reason: "CI postinstall without VOYAVPN_FETCH_RULE_SETS_ON_INSTALL=1", skip: true };
  }
  return { reason: null, skip: false };
}

export async function installRuleSetSeeds({
  env = process.env,
  force = false,
  logger = console,
  postinstall = false,
  repoRoot,
  stage = fetchAndStageRuleSets,
}) {
  const skip = shouldSkipRuleSetInstall({ env, postinstall });
  if (skip.skip) {
    logger.log(`- skipping rule-set seeds: ${skip.reason}`);
    return { status: "skipped" };
  }
  if (!force && verifyStagedRuleSets({ repoRoot }).ok) {
    return { dir: ruleSetSeedDir(repoRoot), status: "already-staged" };
  }
  const result = await stage({ logger, repoRoot });
  return { ...result, status: "staged" };
}
