import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { parseArgs } from "../../lib/args.mjs";
import { repoRootFromScript } from "../../lib/common.mjs";
import {
  defaultEvidencePath,
  isStableChannel,
  normalizeReleaseUrl,
  requiredNonPlaceholderString,
  sourceInputEvidence,
} from "../validation.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const stableChannel = "stable";

const argSpec = {
  "--fixture": { key: "fixture" },
  "--output|--out": { key: "output" },
  "--evidence-out": { key: "evidenceOutput" },
  "--base-url": { key: "baseUrl" },
  "--channel": { key: "channel" },
  "--product": { key: "product" },
};

function parseOptions(argv) {
  return parseArgs(argv, argSpec, {
    fixture: null,
    output: "dist/release/core-assets.json",
    evidenceOutput: null,
    baseUrl: null,
    channel: stableChannel,
    product: "VoyaVPN",
  });
}

function printHelp() {
  console.log(`Usage: pnpm release -- core-assets --fixture <core-assets.json> --out <manifest.json> [options]

Generates a stable CDN core asset manifest. The current stable release does not
publish downloadable core updates; sing-box is bundled as an application seed.
The input must contain an empty assets array. The manifest and release evidence
record that no standalone core downloads are published.

Options:
  --fixture <file>         Core asset fixture JSON input
  --out <file>             Core manifest JSON output path. Default: dist/release/core-assets.json
  --evidence-out <file>    Evidence JSON output path. Default: sibling *.evidence.json
  --base-url <url>         CDN base URL. Stable requires this or VOYAVPN_CDN_BASE_URL
  --channel <name>         Release channel. Default: stable
  --product <name>         Product name recorded in the manifest. Default: VoyaVPN

Nonempty asset lists and unapproved stable CDN base URLs are rejected.`);
}

function normalizeBaseUrl(baseUrl, channel) {
  const value = (baseUrl ?? "").trim();
  if (!value) {
    throw new Error(
      isStableChannel(channel)
        ? "Stable core asset manifest generation requires --base-url or VOYAVPN_CDN_BASE_URL"
        : "Core asset manifest generation requires --base-url or VOYAVPN_CDN_BASE_URL",
    );
  }

  // The dry-run lane generates "stable" metadata against the placeholder `.test`
  // CDN, so local/test hosts stay allowed here.
  return normalizeReleaseUrl(value, {
    allowHttp: true,
    allowTestHosts: true,
    checkHost: isStableChannel(channel),
    label: isStableChannel(channel) ? "Stable CDN base URL" : "CDN base URL",
  });
}

async function main(argv = []) {
  const options = parseOptions(argv);
  if (options.help) {
    printHelp();
    return;
  }
  if (!options.fixture) {
    throw new Error("--fixture is required");
  }

  const fixturePath = resolve(repoRoot, options.fixture);
  const outputPath = resolve(repoRoot, options.output);
  const evidencePath = resolve(repoRoot, options.evidenceOutput ?? defaultEvidencePath(outputPath));
  const baseUrl = normalizeBaseUrl(options.baseUrl ?? process.env.VOYAVPN_CDN_BASE_URL, options.channel);
  const fixture = JSON.parse(await readFile(fixturePath, "utf8"));

  if (!Array.isArray(fixture?.assets)) {
    throw new Error(`${fixturePath} is missing assets[]`);
  }

  if (fixture.assets.length !== 0) {
    throw new Error("Downloadable core assets are not supported; sing-box is bundled with the application");
  }

  const generatedAt = requiredNonPlaceholderString(fixture.generatedAt ?? "1970-01-01T00:00:00.000Z", "generatedAt", "fixture");
  const manifest = {
    productName: options.product,
    manifestVersion: 1,
    channel: options.channel,
    baseUrl,
    generatedAt,
    assets: [],
  };

  const evidence = {
    productName: options.product,
    manifestVersion: manifest.manifestVersion,
    channel: options.channel,
    versions: {},
    baseUrl,
    generatedAt,
    coreManifestPath: outputPath,
    evidencePath,
    sourceInput: sourceInputEvidence(fixturePath, repoRoot, "operator-supplied-core-assets"),
    sourceFixture: fixturePath,
    assetCount: 0,
    targetCount: 0,
    firstStableTargetCount: 0,
    firstStableTargets: [],
    coreTypeCount: 0,
    checksumCount: 0,
    sourceArtifactNames: [],
    validations: {
      urlsDerivedFromBaseUrl: true,
      githubUrlsOnlyInUpstreamReferences: true,
      firstStableMatrixComplete: isStableChannel(options.channel),
      requiredAssetFieldsPresent: true,
    },
    targets: [],
    coreTargets: [],
    assets: [],
  };

  await mkdir(dirname(outputPath), { recursive: true });
  await mkdir(dirname(evidencePath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);

  console.log(`Wrote core asset manifest to ${outputPath}`);
  console.log(`Wrote core asset evidence to ${evidencePath}`);
}

export { main, printHelp };
