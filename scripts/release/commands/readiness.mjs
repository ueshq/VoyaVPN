import { existsSync } from "node:fs";
import { parseArgs } from "../../lib/args.mjs";
import {
  defaultTauriConfig,
  generatedStableUpdaterConfig,
  resolveRepoPath,
  isDryRun,
  normalizeUrl,
  createWorkDir,
} from "./readiness/inputs.mjs";
import { Reporter } from "./readiness/reporter.mjs";
import {
  checkRequiredDocs,
  checkNotices,
  checkCoreSeedPinning,
  checkStableEnvironment,
} from "./readiness/prerequisites.mjs";
import { checkTauriConfig } from "./readiness/config.mjs";
import { scanProductionBlockers } from "./readiness/blockers.mjs";
import { checkGeneratedManifests } from "./readiness/evidence.mjs";

const argSpec = {
  "--mode": { key: "mode" },
  "--cdn-base-url|--base-url": { key: "cdnBaseUrl" },
  "--updates-base-url": { key: "updatesBaseUrl" },
  "--work-dir": { key: "workDir" },
  "--release-artifacts": { key: "releaseArtifacts" },
  "--updater-artifacts": { key: "updaterArtifacts" },
  "--core-assets": { key: "coreAssets" },
  "--release-index": { key: "releaseIndex" },
  "--updater-metadata": { key: "updaterMetadata" },
  "--core-manifest": { key: "coreManifest" },
  "--tauri-config": { key: "tauriConfig" },
};

function parseOptions(argv) {
  const options = parseArgs(argv, argSpec, {
    mode: "dry-run",
    cdnBaseUrl: null,
    updatesBaseUrl: null,
    workDir: null,
    releaseArtifacts: null,
    updaterArtifacts: null,
    coreAssets: null,
    releaseIndex: null,
    updaterMetadata: null,
    coreManifest: null,
    tauriConfig: null,
  });

  if (options.mode !== "dry-run" && options.mode !== "stable") {
    throw new Error("--mode must be dry-run or stable");
  }

  return options;
}

/**
 * Resolves which Tauri config the run scans.
 *
 * The committed base config is deliberately credential-free, so a stable run
 * that scanned it could only ever report an empty pubkey, empty endpoints and
 * disabled updater artifacts. Stable therefore defaults to the generated stable
 * overlay and says how to produce it when it is missing, which is what the
 * runbook has always claimed the bare command does.
 */
export function resolveTauriConfig(options, { configExists = (path) => existsSync(resolveRepoPath(path)) } = {}) {
  if (options.tauriConfig) {
    return options.tauriConfig;
  }
  if (isDryRun(options)) {
    return defaultTauriConfig;
  }
  if (configExists(generatedStableUpdaterConfig)) {
    return generatedStableUpdaterConfig;
  }

  throw new Error(
    `Stable readiness needs the generated stable updater overlay (${generatedStableUpdaterConfig}). ` +
      "Run `pnpm release -- updater-config` first, or pass --tauri-config <overlay>.",
  );
}

function printHelp() {
  console.log(`Usage: pnpm release -- readiness [options]

Runs local release readiness checks for CDN release metadata, updater metadata,
core manifests, release docs, Tauri updater config, and stable-only env inputs.

Options:
  --mode <dry-run|stable>       Readiness mode. Default: dry-run
  --cdn-base-url <url>          CDN base URL. Stable defaults to VOYAVPN_CDN_BASE_URL;
                                dry-run defaults to https://cdn.voyavpn.test/stable
  --updates-base-url <url>      Tauri updater base URL. Defaults to VOYAVPN_UPDATES_BASE_URL,
                                then the CDN base URL
  --work-dir <dir>              Directory for generated check output. Default: OS temp dir
  --release-artifacts <dir>     artifact-manifest root. Dry-run default: tests fixtures;
                                stable default: VOYAVPN_RELEASE_ARTIFACTS_DIR or dist/release/artifacts
  --updater-artifacts <dir>     Signed updater manifest root. Dry-run default: tests fixtures;
                                stable default: VOYAVPN_SIGNED_UPDATER_DIR or dist/release/signed-updater
  --core-assets <file>          Core asset source JSON. Dry-run default: tests fixtures;
                                stable default: VOYAVPN_CORE_ASSETS_FILE or dist/release/core-assets/source-core-assets.json
  --release-index <file>        Existing release-index.json artifact to validate as workflow evidence
  --updater-metadata <file>     Existing latest.json artifact to validate as workflow evidence
  --core-manifest <file>        Existing generated core-assets.json artifact to validate as workflow evidence
  --tauri-config <file>         Tauri config or package-uploaded stable overlay to scan. Non-default
                                paths are merged over apps/desktop/src-tauri/tauri.conf.json

Dry-run mode uses fixture data and does not require signing secrets. Stable mode
fails closed on missing production inputs, placeholder updater keys/signatures,
package-time updater overlay evidence, example URLs,
and GitHub release/download URLs in production surfaces.`);
}

async function main(argv = []) {
  const options = parseOptions(argv);
  if (options.help) {
    printHelp();
    return;
  }
  options.tauriConfig = resolveTauriConfig(options);
  const cdnBaseUrl = normalizeUrl(
    options.cdnBaseUrl ?? process.env.VOYAVPN_CDN_BASE_URL,
    "CDN base URL (VOYAVPN_CDN_BASE_URL or --cdn-base-url)",
    options.mode,
    {
      defaultDryRunUrl: "https://cdn.voyavpn.test/stable",
      requireHttps: options.mode === "stable",
    },
  );
  const updatesBaseUrl = normalizeUrl(
    options.updatesBaseUrl ?? process.env.VOYAVPN_UPDATES_BASE_URL ?? cdnBaseUrl,
    "updater base URL",
    options.mode,
    {
      requireHttps: true,
    },
  );
  const workDir = await createWorkDir(options);
  const reporter = new Reporter(options.mode);

  await checkRequiredDocs(reporter);
  await checkNotices(reporter);
  await checkCoreSeedPinning(reporter);
  await checkStableEnvironment(reporter, options);
  await checkTauriConfig(reporter, options, updatesBaseUrl);
  await scanProductionBlockers(reporter);

  try {
    await checkGeneratedManifests(reporter, options, cdnBaseUrl, updatesBaseUrl, workDir);
  } catch (error) {
    reporter.fail("generated stable metadata", [error.message]);
  }

  reporter.print({ cdnBaseUrl, updatesBaseUrl, workDir });
  if (reporter.hasFailures()) {
    throw new Error("Release readiness checks failed");
  }
}

export { main, printHelp };
