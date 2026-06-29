import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readdir, readFile, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const defaultTauriConfig = "src-tauri/tauri.conf.json";

const stableTargets = [
  { os: "windows", arch: "x64", updater: "windows-x86_64" },
  { os: "windows", arch: "arm64", updater: "windows-aarch64" },
  { os: "macos", arch: "x64", updater: "darwin-x86_64" },
  { os: "macos", arch: "arm64", updater: "darwin-aarch64" },
  { os: "linux", arch: "x64", updater: "linux-x86_64" },
  { os: "linux", arch: "arm64", updater: "linux-aarch64" },
];

const stableCoreTypes = [];
const requiredDocs = [
  "docs/release/packaging.md",
  "docs/release/ci-secrets.md",
  "docs/release/signing-notarization.md",
  "docs/release/os-smoke-matrix.md",
  "docs/release/rollback.md",
  "docs/release/runbook.md",
  "docs/release/diagnostics-privacy.md",
  "docs/release/THIRD_PARTY_NOTICES.md",
  "docs/verification/stable-release-gate.md",
];

const blockerScanFiles = [
  "src-tauri/tauri.conf.json",
  ".github/workflows/release.yml",
  "docs/release/packaging.md",
  "docs/release/ci-secrets.md",
  "docs/release/signing-notarization.md",
  "docs/release/os-smoke-matrix.md",
  "docs/release/rollback.md",
  "docs/release/runbook.md",
  "docs/release/diagnostics-privacy.md",
  "docs/release/THIRD_PARTY_NOTICES.md",
  "docs/verification/stable-release-gate.md",
  "crates/voya-net/src/lib.rs",
  "crates/voya-net/src/update.rs",
];

const urlTextRegex = /\bhttps?:\/\/[^\s"'`<>]+/gi;
const githubDownloadPathRegex = /\/releases\/(?:latest\/)?download\/|\/latest\/download\//i;
const exampleTextRegex = /\bhttps?:\/\/[^\s"'`<>)]*voyavpn\.example[^\s"'`<>)]*|\bvoyavpn\.example\b/i;
const updaterPlaceholderRegex = /\bVOYAVPN_UPDATER_(?:PUBLIC_KEY|SIGNATURE)_PLACEHOLDER[A-Z0-9_]*\b/i;
const productionFieldRegex =
  /(?:^|[\s{[,("'])(?:url|urls|downloadUrl|download_url|download-url|cdnUrl|cdn_url|assetUrl|asset_url|artifactUrl|artifact_url|payloadUrl|payload_url|installerUrl|installer_url|manualDownloadUrl|manual_download_url|releaseIndexUrl|release_index_url|latestJsonUrl|latest_json_url|baseUrl|base_url|updatesBaseUrl|updates_base_url|endpoint|endpoints|downloadUrlTemplate|download_url_template|urlTemplate|url_template)["']?\s*[:=]|(?:^|\s)VOYAVPN_(?:CDN_BASE_URL|UPDATES_BASE_URL|DIAGNOSTICS_ENDPOINT)\s*=/i;
const productionCliUrlRegex = /--(?:cdn-base-url|updates-base-url|diagnostics-endpoint|base-url)\s+\S+/i;
const productionTemplateContextRegex =
  /\b(?:ReleasePackage|AssetTemplates|downloadTemplates|download_templates|downloadUrlTemplate|download_url_template|urlTemplate|url_template|templates)\b/i;
const sourceEvidenceContextRegex =
  /\b(?:upstreamUrl|upstream_url|upstream|sourceUrl|source_url|SOURCE_URL|sourceBundleDir|sourceManifests|source reference|source evidence|release_api_url|release_url|html_url|repository|homepage|licenseUrl|license_url|UpstreamReleaseEvidence|UpstreamAssetTemplates|asset_templates)\b/i;
const guardOrDefensiveContextRegex =
  /\b(?:forbidden|rejects?|allowed only|must not|should not|contains|includes|placeholder\.test|throw new Error|expect_err|assert|no `?voyavpn\.example|no .*github|validation fails|fails when)\b/i;

function parseArgs(argv) {
  const options = {
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
    diagnosticsEndpoint: null,
    tauriConfig: defaultTauriConfig,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    const next = () => {
      const value = argv[index + 1];
      if (!value || value.startsWith("--")) {
        throw new Error(`${arg} requires a value`);
      }
      index += 1;
      return value;
    };

    switch (arg) {
      case "--mode":
        options.mode = next();
        break;
      case "--cdn-base-url":
      case "--base-url":
        options.cdnBaseUrl = next();
        break;
      case "--updates-base-url":
        options.updatesBaseUrl = next();
        break;
      case "--work-dir":
        options.workDir = next();
        break;
      case "--release-artifacts":
        options.releaseArtifacts = next();
        break;
      case "--updater-artifacts":
        options.updaterArtifacts = next();
        break;
      case "--core-assets":
        options.coreAssets = next();
        break;
      case "--release-index":
        options.releaseIndex = next();
        break;
      case "--updater-metadata":
        options.updaterMetadata = next();
        break;
      case "--core-manifest":
        options.coreManifest = next();
        break;
      case "--diagnostics-endpoint":
        options.diagnosticsEndpoint = next();
        break;
      case "--tauri-config":
        options.tauriConfig = next();
        break;
      case "--help":
        printHelp();
        process.exit(0);
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }

  if (options.mode !== "dry-run" && options.mode !== "stable") {
    throw new Error("--mode must be dry-run or stable");
  }

  return options;
}

function printHelp() {
  console.log(`Usage: node scripts/check-release-readiness.mjs [options]

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
  --diagnostics-endpoint <url>  Approved diagnostics ingest endpoint. Stable defaults to
                                VOYAVPN_DIAGNOSTICS_ENDPOINT; dry-run does not require one
  --tauri-config <file>         Tauri config or package-uploaded stable overlay to scan. Non-default
                                paths are merged over src-tauri/tauri.conf.json

Dry-run mode uses fixture data and does not require signing secrets. Stable mode
fails closed on missing production inputs, placeholder updater keys/signatures,
diagnostics endpoint config, package-time updater overlay evidence, example URLs,
and GitHub release/download URLs in production surfaces.`);
}

function isDryRun(options) {
  return options.mode === "dry-run";
}

function displayPath(path) {
  return relative(repoRoot, path).replaceAll("\\", "/") || ".";
}

function resolveRepoPath(path) {
  return resolve(repoRoot, path);
}

function stableInputPath(options, explicit, envName, fallback) {
  return explicit ?? process.env[envName] ?? fallback;
}

function stableInputPathAny(explicit, envNames, fallback) {
  if (explicit) {
    return explicit;
  }
  for (const envName of envNames) {
    if (process.env[envName]) {
      return process.env[envName];
    }
  }
  return fallback;
}

function assertStableEvidencePath(options, label, path) {
  if (isDryRun(options)) {
    return;
  }

  const normalized = relative(repoRoot, resolve(repoRoot, path)).replaceAll("\\", "/");
  if (normalized === "tests/fixtures" || normalized.startsWith("tests/fixtures/")) {
    throw new Error(`${label} must not use tests/fixtures in stable mode: ${normalized}`);
  }
}

async function createWorkDir(options) {
  if (options.workDir) {
    return resolve(repoRoot, options.workDir);
  }
  return mkdtemp(join(tmpdir(), "voyavpn-readiness-"));
}

function isForbiddenStableHost(hostname, mode) {
  const host = hostname.toLowerCase();
  const forbidden =
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
    host.includes("placeholder");

  if (forbidden) {
    return true;
  }

  return (
    mode === "stable" &&
    (host === "localhost" || host === "127.0.0.1" || host === "::1" || host.endsWith(".test"))
  );
}

function normalizeUrl(value, label, mode, { defaultDryRunUrl = null, requireHttps = false } = {}) {
  const resolvedValue = (value ?? (mode === "dry-run" ? defaultDryRunUrl : null) ?? "").trim();
  if (!resolvedValue) {
    throw new Error(`${label} is required`);
  }

  let parsed;
  try {
    parsed = new URL(resolvedValue);
  } catch {
    throw new Error(`${label} is not a valid URL: ${resolvedValue}`);
  }

  if (requireHttps && parsed.protocol !== "https:") {
    throw new Error(`${label} must use https: ${resolvedValue}`);
  }
  if (!requireHttps && parsed.protocol !== "https:" && parsed.protocol !== "http:") {
    throw new Error(`${label} must use http or https: ${resolvedValue}`);
  }
  if (isForbiddenStableHost(parsed.hostname, mode)) {
    throw new Error(`${label} must not use example, GitHub, placeholder, localhost, or .test hosts: ${resolvedValue}`);
  }

  parsed.hash = "";
  parsed.search = "";
  return parsed.toString().replace(/\/+$/g, "");
}

function normalizeDiagnosticsEndpoint(value, mode) {
  const resolvedValue = (value ?? "").trim();
  if (!resolvedValue) {
    if (mode === "dry-run") {
      return null;
    }
    throw new Error("diagnostics endpoint (VOYAVPN_DIAGNOSTICS_ENDPOINT or --diagnostics-endpoint) is required");
  }

  let parsed;
  try {
    parsed = new URL(resolvedValue);
  } catch {
    throw new Error("diagnostics endpoint is not a valid URL");
  }

  if (parsed.protocol !== "https:") {
    throw new Error("diagnostics endpoint must use https");
  }
  if (parsed.username || parsed.password) {
    throw new Error("diagnostics endpoint must not include credentials");
  }
  if (parsed.search || parsed.hash) {
    throw new Error("diagnostics endpoint must not include query strings or fragments");
  }
  if (isForbiddenStableHost(parsed.hostname, mode) || isIpHost(parsed.hostname)) {
    throw new Error("diagnostics endpoint must not use example, GitHub, placeholder, localhost, .test, or IP hosts");
  }

  parsed.hash = "";
  parsed.search = "";
  return parsed.toString();
}

function isIpHost(hostname) {
  const host = hostname.toLowerCase();
  if (host.includes(":")) {
    return true;
  }
  return /^\d{1,3}(?:\.\d{1,3}){3}$/.test(host);
}

function placeholderText(value) {
  return (
    !value ||
    /placeholder|replace_before_release|replace-before-release|changeme|\btodo\b|\btbd\b|voyavpn\.example/i.test(
      String(value),
    )
  );
}

function readJson(path) {
  return readFile(path, "utf8").then((text) => JSON.parse(text));
}

function sha256Text(value) {
  return createHash("sha256").update(value).digest("hex");
}

async function sha256File(path) {
  return sha256Text(await readFile(path));
}

async function walkArtifactManifests(root) {
  let rootStat;
  try {
    rootStat = await stat(root);
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return [];
    }
    throw error;
  }

  if (rootStat.isFile()) {
    return basename(root) === "artifact-manifest.json" ? [root] : [];
  }

  const entries = await readdir(root, { withFileTypes: true });
  const manifests = [];

  for (const entry of entries) {
    if (entry.isSymbolicLink()) {
      continue;
    }

    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      manifests.push(...(await walkArtifactManifests(path)));
    } else if (entry.isFile() && entry.name === "artifact-manifest.json") {
      manifests.push(path);
    }
  }

  return manifests.sort((left, right) => left.localeCompare(right));
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function mergeConfig(base, overlay) {
  const merged = { ...base };
  for (const [key, value] of Object.entries(overlay)) {
    if (isPlainObject(value) && isPlainObject(merged[key])) {
      merged[key] = mergeConfig(merged[key], value);
    } else {
      merged[key] = value;
    }
  }
  return merged;
}

function isCredentialFreeUpdaterConfig(updater) {
  return (
    updater &&
    typeof updater === "object" &&
    placeholderText(updater.pubkey) &&
    Array.isArray(updater.endpoints) &&
    updater.endpoints.length === 0
  );
}

async function loadTauriConfig(options) {
  const basePath = resolveRepoPath(defaultTauriConfig);
  const requestedPath = resolveRepoPath(options.tauriConfig);
  const baseConfig = await readJson(basePath);

  if (requestedPath === basePath) {
    return {
      config: baseConfig,
      label: displayPath(basePath),
      isDefaultConfig: true,
    };
  }

  const overlay = await readJson(requestedPath);
  return {
    config: mergeConfig(baseConfig, overlay),
    label: `${displayPath(basePath)} + ${displayPath(requestedPath)}`,
    isDefaultConfig: false,
  };
}

function forbiddenSerialized(value) {
  const text = JSON.stringify(value).toLowerCase();
  return text.includes("voyavpn.example") || text.includes("placeholder") || text.includes("github.com");
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function lineSummary(lines, maxLines = 6) {
  const selected = lines.slice(0, maxLines).map((line) => `       ${line}`);
  if (lines.length > maxLines) {
    selected.push(`       ... ${lines.length - maxLines} more`);
  }
  return selected;
}

class Reporter {
  constructor(mode) {
    this.mode = mode;
    this.records = [];
  }

  pass(name, details = []) {
    this.records.push({ status: "PASS", name, details });
  }

  warn(name, details = []) {
    this.records.push({ status: "WARN", name, details });
  }

  fail(name, details = []) {
    this.records.push({ status: "FAIL", name, details });
  }

  blocker(name, details = []) {
    if (this.mode === "stable") {
      this.fail(name, details);
    } else {
      this.warn(`${name} (stable blocker, allowed in dry-run)`, details);
    }
  }

  print({ cdnBaseUrl, updatesBaseUrl, diagnosticsEndpoint, workDir }) {
    console.log(`VoyaVPN release readiness (${this.mode})`);
    console.log(`CDN base URL: ${cdnBaseUrl}`);
    console.log(`Updater base URL: ${updatesBaseUrl}`);
    console.log(`Diagnostics endpoint: ${diagnosticsEndpoint ? "configured" : "not configured"}`);
    console.log(`Generated output: ${workDir}`);
    console.log("");

    for (const record of this.records) {
      console.log(`[${record.status}] ${record.name}`);
      for (const detail of record.details) {
        console.log(`       ${detail}`);
      }
    }

    const counts = this.records.reduce(
      (current, record) => {
        current[record.status] += 1;
        return current;
      },
      { PASS: 0, WARN: 0, FAIL: 0 },
    );

    console.log("");
    if (counts.FAIL > 0) {
      console.log(`Readiness result: FAIL (${counts.PASS} passed, ${counts.WARN} warnings, ${counts.FAIL} failed)`);
    } else {
      console.log(`Readiness result: PASS (${counts.PASS} passed, ${counts.WARN} warnings, ${counts.FAIL} failed)`);
    }
  }

  hasFailures() {
    return this.records.some((record) => record.status === "FAIL");
  }
}

async function checkRequiredDocs(reporter) {
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

async function checkNotices(reporter) {
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

async function checkTauriConfig(reporter, options, updatesBaseUrl) {
  const { config, label, isDefaultConfig } = await loadTauriConfig(options);
  const updater = config.plugins?.updater;
  const bundle = config.bundle ?? {};
  const detailsPrefix = label;
  const defaultDryRunConfig = isDryRun(options) && isDefaultConfig;

  if (!updater || typeof updater !== "object") {
    if (defaultDryRunConfig) {
      reporter.pass("Tauri updater config", [
        `${detailsPrefix}: dry-run uses the credential-free base config; stable mode validates the generated updater overlay`,
      ]);
    } else {
      reporter.blocker("Tauri updater config", [`${detailsPrefix}: plugins.updater is missing`]);
    }
  } else if (defaultDryRunConfig && isCredentialFreeUpdaterConfig(updater)) {
    reporter.pass("Tauri updater config", [
      `${detailsPrefix}: dry-run uses an empty credential-free updater config; stable mode validates the generated updater overlay`,
    ]);
  } else {
    if (placeholderText(updater.pubkey)) {
      reporter.blocker("Tauri updater public key", [`${detailsPrefix}: plugins.updater.pubkey is empty or a placeholder`]);
    } else if (String(updater.pubkey).trim().length < 32) {
      reporter.blocker("Tauri updater public key", [`${detailsPrefix}: plugins.updater.pubkey is too short`]);
    } else {
      reporter.pass("Tauri updater public key", [`${detailsPrefix}: public key is non-placeholder`]);
    }

    if (!Array.isArray(updater.endpoints) || updater.endpoints.length === 0) {
      reporter.blocker("Tauri updater endpoints", [`${detailsPrefix}: plugins.updater.endpoints is missing or empty`]);
    } else {
      const updateHost = new URL(updatesBaseUrl).hostname.toLowerCase();
      const badEndpoints = [];
      for (const endpoint of updater.endpoints) {
        const endpointText = String(endpoint);
        if (placeholderText(endpointText)) {
          badEndpoints.push(`${endpointText} uses an example or placeholder URL`);
          continue;
        }

        try {
          const parsed = new URL(
            endpointText
              .replaceAll("{{target}}", "darwin")
              .replaceAll("{{arch}}", "aarch64")
              .replaceAll("{{current_version}}", "0.1.0"),
          );
          if (parsed.protocol !== "https:") {
            badEndpoints.push(`${endpointText} does not use https`);
          }
          if (options.mode === "stable" && parsed.hostname.toLowerCase() !== updateHost) {
            badEndpoints.push(`${endpointText} does not match updater base host ${updateHost}`);
          }
          if (options.mode === "stable" && !parsed.toString().startsWith(`${updatesBaseUrl}/`)) {
            badEndpoints.push(`${endpointText} is not derived from updater base URL ${updatesBaseUrl}`);
          }
        } catch {
          badEndpoints.push(`${endpointText} is not parseable as a URL template`);
        }
      }

      if (badEndpoints.length > 0) {
        reporter.blocker("Tauri updater endpoints", badEndpoints);
      } else {
        reporter.pass("Tauri updater endpoints", [`${updater.endpoints.length} endpoint template(s) use stable URL rules`]);
      }
    }
  }

  if (bundle.createUpdaterArtifacts === true) {
    reporter.pass("Tauri updater artifact flag", [`${detailsPrefix}: bundle.createUpdaterArtifacts is enabled`]);
  } else if (defaultDryRunConfig) {
    reporter.pass("Tauri updater artifact flag", [
      `${detailsPrefix}: disabled for credential-free dry runs; stable overlay must enable updater artifacts`,
    ]);
  } else {
    reporter.blocker("Tauri updater artifact flag", [
      `${detailsPrefix}: bundle.createUpdaterArtifacts is not enabled; pass a stable overlay with updater artifacts enabled`,
    ]);
  }

  const resources = bundle.resources ?? {};
  if (resources["../docs/release/THIRD_PARTY_NOTICES.md"] === "release/THIRD_PARTY_NOTICES.md") {
    reporter.pass("bundled notices resource", [`${detailsPrefix}: THIRD_PARTY_NOTICES.md is bundled`]);
  } else {
    reporter.fail("bundled notices resource", [`${detailsPrefix}: release notices resource is missing from bundle.resources`]);
  }
}

async function checkStableEnvironment(reporter, options, cdnBaseUrl, updatesBaseUrl) {
  if (isDryRun(options)) {
    reporter.pass("stable-only secrets", ["dry-run mode does not require signing, notarization, or publication secrets"]);
    return;
  }

  const missing = [];

  if (!options.cdnBaseUrl && !process.env.VOYAVPN_CDN_BASE_URL) {
    missing.push("VOYAVPN_CDN_BASE_URL or --cdn-base-url");
  }
  if (!options.updatesBaseUrl && !process.env.VOYAVPN_UPDATES_BASE_URL && updatesBaseUrl !== cdnBaseUrl) {
    missing.push("VOYAVPN_UPDATES_BASE_URL or --updates-base-url");
  }
  if (!options.diagnosticsEndpoint && !process.env.VOYAVPN_DIAGNOSTICS_ENDPOINT) {
    missing.push("VOYAVPN_DIAGNOSTICS_ENDPOINT or --diagnostics-endpoint");
  }
  if (!process.env.TAURI_SIGNING_PRIVATE_KEY && !process.env.TAURI_SIGNING_PRIVATE_KEY_PATH) {
    missing.push("TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH");
  }

  for (const name of ["APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID"]) {
    if (!process.env[name]) {
      missing.push(name);
    }
  }

  for (const name of ["WINDOWS_CERTIFICATE_BASE64", "WINDOWS_CERTIFICATE_PASSWORD"]) {
    if (!process.env[name]) {
      missing.push(name);
    }
  }

  if (missing.length > 0) {
    reporter.fail("stable required env inputs", missing.map((name) => `missing: ${name}`));
    return;
  }

  reporter.pass("stable required env inputs", [
    "CDN/updater base URLs, diagnostics endpoint, and signing env names are present; secret values are not printed",
  ]);
}

async function checkDiagnosticsEndpoint(reporter, options, diagnosticsEndpoint) {
  if (!diagnosticsEndpoint) {
    reporter.pass("diagnostics endpoint config", ["dry-run mode does not require diagnostics delivery config"]);
    return;
  }

  reporter.pass("diagnostics endpoint config", [
    "approved HTTPS endpoint is configured; endpoint value is not printed in readiness output",
  ]);
}

function trimUrlText(value) {
  return value.replace(/[),.;\]}]+$/g, "");
}

function extractUrls(line) {
  return [...line.matchAll(urlTextRegex)].map((match) => trimUrlText(match[0]));
}

function isGithubStableHost(hostname) {
  const host = hostname.toLowerCase();
  return (
    host === "github.com" ||
    host.endsWith(".github.com") ||
    host === "githubusercontent.com" ||
    host.endsWith(".githubusercontent.com") ||
    host === "github.io" ||
    host.endsWith(".github.io")
  );
}

function isGithubProductionUrl(url) {
  try {
    const parsed = new URL(url.replaceAll("{tag}", "v1.0.0").replaceAll("{version}", "1.0.0"));
    return isGithubStableHost(parsed.hostname);
  } catch {
    return /github\.com|githubusercontent\.com|github\.io/i.test(url);
  }
}

function isGithubReleaseDownloadUrl(url) {
  return isGithubProductionUrl(url) && githubDownloadPathRegex.test(url);
}

function isSourceEvidenceContext(context) {
  return sourceEvidenceContextRegex.test(context) || /SOURCE_URL/i.test(context);
}

function isSourceEvidenceRole(line, context) {
  if (sourceEvidenceContextRegex.test(line) || /SOURCE_URL/i.test(line)) {
    return true;
  }
  if (productionFieldRegex.test(line) || productionCliUrlRegex.test(line)) {
    return false;
  }
  return isSourceEvidenceContext(context);
}

function isGuardOrDefensiveContext(context) {
  return guardOrDefensiveContextRegex.test(context);
}

function firstRustTestLine(lines) {
  return lines.findIndex((line) => /^\s*(?:pub\s+)?mod\s+tests\s*\{/.test(line));
}

function isTestSurface(file, lineIndex, firstRustTestIndex) {
  const normalized = file.replaceAll("\\", "/");
  return (
    normalized.includes("/tests/") ||
    normalized.startsWith("tests/") ||
    /\.(?:test|spec)\.[cm]?[jt]sx?$/.test(normalized) ||
    (normalized.endsWith(".rs") && firstRustTestIndex !== -1 && lineIndex >= firstRustTestIndex)
  );
}

function scanContext(lines, lineIndex) {
  return lines.slice(Math.max(0, lineIndex - 4), lineIndex + 1).join("\n");
}

function hasProductionUrlRole(line, context, url) {
  if (isSourceEvidenceRole(line, context)) {
    return false;
  }
  if (productionFieldRegex.test(line) || productionFieldRegex.test(context)) {
    return true;
  }
  if (productionCliUrlRegex.test(line) || productionCliUrlRegex.test(context)) {
    return true;
  }
  if (productionTemplateContextRegex.test(context)) {
    return true;
  }
  return isGithubReleaseDownloadUrl(url);
}

function classifyProductionBlocker(line, context) {
  if (isGuardOrDefensiveContext(context)) {
    return null;
  }

  if (updaterPlaceholderRegex.test(line) && !isSourceEvidenceRole(line, context)) {
    return "updater placeholder";
  }

  if (exampleTextRegex.test(line)) {
    return productionFieldRegex.test(line) || productionCliUrlRegex.test(line) ? "example production URL" : null;
  }

  for (const url of extractUrls(line)) {
    if (!hasProductionUrlRole(line, context, url)) {
      continue;
    }
    if (isGithubProductionUrl(url)) {
      return isGithubReleaseDownloadUrl(url) ? "GitHub production download URL" : "GitHub production URL";
    }
    if (/voyavpn\.example/i.test(url)) {
      return "example production URL";
    }
    if (/placeholder/i.test(url)) {
      return "placeholder production URL";
    }
  }

  return null;
}

export function findProductionBlockersInText(file, text) {
  const matches = [];
  const lines = text.split(/\r?\n/);
  const firstRustTestIndex = file.endsWith(".rs") ? firstRustTestLine(lines) : -1;

  lines.forEach((line, index) => {
    if (isTestSurface(file, index, firstRustTestIndex)) {
      return;
    }

    const context = scanContext(lines, index);
    const label = classifyProductionBlocker(line, context);
    if (label) {
      matches.push(`${file}:${index + 1}: ${label}: ${line.trim()}`);
    }
  });

  return matches;
}

async function scanProductionBlockers(reporter) {
  const matches = [];
  for (const file of blockerScanFiles) {
    const path = resolveRepoPath(file);
    let text;
    try {
      text = await readFile(path, "utf8");
    } catch (error) {
      if (error && error.code === "ENOENT") {
        continue;
      }
      throw error;
    }

    matches.push(...findProductionBlockersInText(file, text));
  }

  if (matches.length > 0) {
    reporter.blocker("production blocker scan", lineSummary(matches, 10));
    return;
  }

  reporter.pass("production blocker scan", [
    "no forbidden production URL fields, updater placeholders, or GitHub production download templates found",
  ]);
}

async function runGenerator(script, args, env) {
  const result = await execFileAsync(process.execPath, [resolveRepoPath(script), ...args], {
    cwd: repoRoot,
    env: { ...process.env, ...env },
    maxBuffer: 10 * 1024 * 1024,
  });

  return [...result.stdout.trim().split(/\r?\n/), ...result.stderr.trim().split(/\r?\n/)].filter(Boolean);
}

function validateReleaseIndex(index, cdnBaseUrl) {
  assert(index.productName === "VoyaVPN", "release index productName must be VoyaVPN");
  assert(index.channel === "stable", "release index channel must be stable");
  assert(index.baseUrl === cdnBaseUrl, "release index baseUrl must match readiness CDN base URL");
  assert(Array.isArray(index.artifacts) && index.artifacts.length > 0, "release index artifacts[] must be non-empty");
  assert(!forbiddenSerialized(index), "release index contains placeholder, example, or GitHub content");

  const present = new Set(index.artifacts.map((artifact) => `${artifact.target}/${artifact.arch}`));
  const missing = stableTargets
    .map((target) => `${target.os}/${target.arch}`)
    .filter((target) => !present.has(target));
  assert(missing.length === 0, `release index is missing first-stable target(s): ${missing.join(", ")}`);

  for (const artifact of index.artifacts) {
    assert(artifact.url?.startsWith(`${cdnBaseUrl}/`), `release index URL is not CDN-derived: ${artifact.url}`);
    assert(Number.isInteger(artifact.bytes) && artifact.bytes > 0, `release index artifact has invalid bytes: ${artifact.name}`);
    assert(/^[a-f0-9]{64}$/i.test(artifact.sha256 ?? ""), `release index artifact has invalid sha256: ${artifact.name}`);
  }
}

function validateUpdaterMetadata(latest, updatesBaseUrl) {
  assert(typeof latest.version === "string" && latest.version.length > 0, "latest.json version is missing");
  assert(typeof latest.pub_date === "string" && latest.pub_date.length > 0, "latest.json pub_date is missing");
  assert(latest.platforms && typeof latest.platforms === "object", "latest.json platforms object is missing");
  assert(!forbiddenSerialized(latest), "latest.json contains placeholder, example, or GitHub content");

  const keys = Object.keys(latest.platforms).sort((left, right) => left.localeCompare(right));
  const missing = stableTargets.map((target) => target.updater).filter((target) => !keys.includes(target));
  assert(missing.length === 0, `latest.json is missing first-stable updater target(s): ${missing.join(", ")}`);

  for (const [target, platform] of Object.entries(latest.platforms)) {
    assert(platform.url?.startsWith(`${updatesBaseUrl}/`), `updater URL for ${target} is not base-url-derived`);
    assert(!placeholderText(platform.signature), `updater signature for ${target} is a placeholder`);
    assert(String(platform.signature).length >= 32, `updater signature for ${target} is too short`);
  }
}

function validateCoreManifest(manifest, cdnBaseUrl) {
  assert(manifest.productName === "VoyaVPN", "core manifest productName must be VoyaVPN");
  assert(manifest.channel === "stable", "core manifest channel must be stable");
  assert(manifest.baseUrl === cdnBaseUrl, "core manifest baseUrl must match readiness CDN base URL");
  assert(Array.isArray(manifest.assets), "core manifest assets[] must be an array");

  const present = new Set(manifest.assets.map((asset) => `${asset.coreType}/${asset.os}/${asset.arch}`));
  const missing = [];
  for (const coreType of stableCoreTypes) {
    for (const target of stableTargets) {
      const key = `${coreType}/${target.os}/${target.arch}`;
      if (!present.has(key)) {
        missing.push(key);
      }
    }
  }
  assert(missing.length === 0, `core manifest is missing first-stable asset(s): ${missing.join(", ")}`);

  for (const asset of manifest.assets) {
    assert(stableCoreTypes.includes(asset.coreType), `core asset type is not supported in stable releases: ${asset.coreType}`);
    assert(asset.url?.startsWith(`${cdnBaseUrl}/`), `core asset URL is not CDN-derived: ${asset.name}`);
    assert(!/github\.com|voyavpn\.example|placeholder/i.test(asset.url), `core asset production URL is forbidden: ${asset.url}`);
    assert(Number.isInteger(asset.bytes) && asset.bytes > 0, `core asset has invalid bytes: ${asset.name}`);
    assert(/^[a-f0-9]{64}$/i.test(asset.sha256 ?? ""), `core asset has invalid sha256: ${asset.name}`);
    assert(Array.isArray(asset.executableCandidates) && asset.executableCandidates.length > 0, `core asset has no executable candidates: ${asset.name}`);
    assert(typeof asset.upstreamUrl === "string" && asset.upstreamUrl.length > 0, `core asset has no upstream URL: ${asset.name}`);
  }
}

function safeRelativePath(value, context) {
  if (!value || typeof value !== "string") {
    throw new Error(`${context} path is missing`);
  }

  const normalized = value.replaceAll("\\", "/");
  if (normalized.startsWith("/") || normalized.split("/").some((segment) => segment === "..")) {
    throw new Error(`${context} path is unsafe: ${value}`);
  }
  return normalized;
}

export function validateStableUpdaterConfigMetadata(metadata, { updatesBaseUrl, updaterPublicKey, label }) {
  assert(metadata && typeof metadata === "object", `${label} stableUpdaterConfig is missing`);
  assert(/^[a-f0-9]{64}$/i.test(metadata.sha256 ?? ""), `${label} stableUpdaterConfig.sha256 is invalid`);
  assert(metadata.createUpdaterArtifacts === true, `${label} stable updater artifacts flag is not enabled`);

  const expectedPubkeySha256 = sha256Text(String(updaterPublicKey ?? "").trim());
  assert(
    metadata.pubkeySha256 === expectedPubkeySha256,
    `${label} stable updater public key hash does not match VOYAVPN_UPDATER_PUBLIC_KEY`,
  );

  assert(Array.isArray(metadata.endpoints) && metadata.endpoints.length > 0, `${label} updater endpoints are missing`);
  const expectedEndpoint = `${updatesBaseUrl}/latest.json`;
  for (const endpoint of metadata.endpoints) {
    assert(endpoint === expectedEndpoint, `${label} updater endpoint does not match readiness base URL: ${endpoint}`);
  }

  if (metadata.path !== undefined) {
    safeRelativePath(metadata.path, `${label} stableUpdaterConfig`);
  }
}

async function checkStableUpdaterConfigEvidence(reporter, options, roots, updatesBaseUrl, expectedConfigSha256) {
  if (isDryRun(options)) {
    reporter.pass("packaged updater config evidence", [
      "dry-run mode does not require package-time stable updater overlay evidence",
    ]);
    return;
  }

  const updaterPublicKey = process.env.VOYAVPN_UPDATER_PUBLIC_KEY ?? process.env.TAURI_UPDATER_PUBLIC_KEY ?? "";
  const manifestPaths = [];
  for (const root of roots) {
    manifestPaths.push(...(await walkArtifactManifests(resolveRepoPath(root))));
  }

  if (manifestPaths.length === 0) {
    reporter.fail("packaged updater config evidence", ["no artifact-manifest.json files found"]);
    return;
  }

  const failures = [];
  const overlayHashes = new Set();
  const copiedOverlayHashes = new Set();

  for (const manifestPath of manifestPaths) {
    const label = displayPath(manifestPath);
    const manifest = await readJson(manifestPath);
    try {
      validateStableUpdaterConfigMetadata(manifest.stableUpdaterConfig, {
        updatesBaseUrl,
        updaterPublicKey,
        label,
      });
      overlayHashes.add(manifest.stableUpdaterConfig.sha256);

      if (manifest.stableUpdaterConfig.path) {
        const copiedPath = resolve(dirname(manifestPath), safeRelativePath(manifest.stableUpdaterConfig.path, label));
        const copiedHash = await sha256File(copiedPath);
        copiedOverlayHashes.add(copiedHash);
        if (copiedHash !== manifest.stableUpdaterConfig.sha256) {
          failures.push(`${label} stable updater config copy hash does not match artifact-manifest.json`);
        }
      }
    } catch (error) {
      failures.push(error.message);
    }
  }

  if (overlayHashes.size > 1) {
    failures.push(`package targets used different stable updater overlay hashes: ${[...overlayHashes].join(", ")}`);
  }
  if (expectedConfigSha256 && !overlayHashes.has(expectedConfigSha256)) {
    failures.push(
      `checked Tauri updater config hash ${expectedConfigSha256} does not match package-time overlay hash ${[
        ...overlayHashes,
      ].join(", ")}`,
    );
  }
  if (copiedOverlayHashes.size > 1) {
    failures.push(`package targets uploaded different stable updater overlay files: ${[...copiedOverlayHashes].join(", ")}`);
  }

  if (failures.length > 0) {
    reporter.fail("packaged updater config evidence", lineSummary(failures, 10));
    return;
  }

  reporter.pass("packaged updater config evidence", [
    `validated ${manifestPaths.length} artifact manifest(s)`,
    `overlay sha256: ${[...overlayHashes][0]}`,
    ...(expectedConfigSha256 ? ["checked Tauri config hash matches package-time overlay"] : []),
  ]);
}

async function checkGeneratedManifests(reporter, options, cdnBaseUrl, updatesBaseUrl, workDir) {
  const releaseArtifacts = stableInputPath(
    options,
    options.releaseArtifacts,
    "VOYAVPN_RELEASE_ARTIFACTS_DIR",
    isDryRun(options) ? "tests/fixtures/release/artifacts" : "dist/release/artifacts",
  );
  const updaterArtifacts = stableInputPath(
    options,
    options.updaterArtifacts,
    "VOYAVPN_SIGNED_UPDATER_DIR",
    isDryRun(options) ? "tests/fixtures/release/signed-updater" : "dist/release/signed-updater",
  );
  const coreAssets = stableInputPathAny(
    options.coreAssets,
    ["VOYAVPN_CORE_ASSETS_FILE", "VOYAVPN_CORE_ASSETS_FIXTURE"],
    isDryRun(options) ? "tests/fixtures/release/core-assets.json" : "dist/release/core-assets/source-core-assets.json",
  );

  const releaseIndexOut = join(workDir, "release-index.json");
  const latestOut = join(workDir, "latest.json");
  const coreManifestOut = join(workDir, "core-assets.json");
  const env = {
    VOYAVPN_CDN_BASE_URL: cdnBaseUrl,
    VOYAVPN_UPDATES_BASE_URL: updatesBaseUrl,
  };
  if (isDryRun(options)) {
    const fixtureUpdaterPublicKey = (
      await readFile(resolveRepoPath("tests/fixtures/release/updater-signing/public.key"), "utf8")
    ).trim();
    env.VOYAVPN_UPDATER_PUBLIC_KEY = fixtureUpdaterPublicKey;
    env.TAURI_UPDATER_PUBLIC_KEY = fixtureUpdaterPublicKey;
  }

  assertStableEvidencePath(options, "release artifacts", releaseArtifacts);
  assertStableEvidencePath(options, "signed updater artifacts", updaterArtifacts);
  assertStableEvidencePath(options, "core asset source", coreAssets);
  const expectedConfigSha256 = isDryRun(options) ? null : await sha256File(resolveRepoPath(options.tauriConfig));
  await checkStableUpdaterConfigEvidence(
    reporter,
    options,
    [...new Set([releaseArtifacts, updaterArtifacts])],
    updatesBaseUrl,
    expectedConfigSha256,
  );
  if (options.releaseIndex) {
    assertStableEvidencePath(options, "release index evidence", options.releaseIndex);
    const releaseIndexEvidence = await readJson(resolveRepoPath(options.releaseIndex));
    validateReleaseIndex(releaseIndexEvidence, cdnBaseUrl);
    reporter.pass("workflow CDN staging metadata", [`validated ${options.releaseIndex}`]);
  }
  if (options.updaterMetadata) {
    assertStableEvidencePath(options, "updater metadata evidence", options.updaterMetadata);
    const updaterMetadataEvidence = await readJson(resolveRepoPath(options.updaterMetadata));
    validateUpdaterMetadata(updaterMetadataEvidence, updatesBaseUrl);
    reporter.pass("workflow updater metadata", [`validated ${options.updaterMetadata}`]);
  }
  if (options.coreManifest) {
    assertStableEvidencePath(options, "core manifest evidence", options.coreManifest);
    const coreManifestEvidence = await readJson(resolveRepoPath(options.coreManifest));
    validateCoreManifest(coreManifestEvidence, cdnBaseUrl);
    reporter.pass("workflow core manifest metadata", [`validated ${options.coreManifest}`]);
  }

  const releaseOutput = await runGenerator(
    "scripts/release-index.mjs",
    ["--input", releaseArtifacts, "--out", releaseIndexOut, "--base-url", cdnBaseUrl, "--channel", "stable"],
    env,
  );
  const releaseIndex = await readJson(releaseIndexOut);
  validateReleaseIndex(releaseIndex, cdnBaseUrl);
  reporter.pass("stable release index manifest", [
    `generated ${displayPath(releaseIndexOut)}`,
    `artifacts: ${releaseIndex.artifacts.length}`,
    ...lineSummary(releaseOutput, 2).map((line) => line.trim()),
  ]);

  const updaterOutput = await runGenerator(
    "scripts/release-updater-metadata.mjs",
    ["--input", updaterArtifacts, "--out", latestOut, "--channel", "stable", "--base-url", updatesBaseUrl],
    env,
  );
  const latest = await readJson(latestOut);
  validateUpdaterMetadata(latest, updatesBaseUrl);
  reporter.pass("stable updater metadata", [
    `generated ${displayPath(latestOut)}`,
    `platforms: ${Object.keys(latest.platforms).length}`,
    ...lineSummary(updaterOutput, 2).map((line) => line.trim()),
  ]);

  const coreOutput = await runGenerator(
    "scripts/core-assets.mjs",
    ["--fixture", coreAssets, "--out", coreManifestOut, "--base-url", cdnBaseUrl, "--channel", "stable"],
    env,
  );
  const coreManifest = await readJson(coreManifestOut);
  validateCoreManifest(coreManifest, cdnBaseUrl);
  reporter.pass("stable core asset manifest", [
    `generated ${displayPath(coreManifestOut)}`,
    `assets: ${coreManifest.assets.length}`,
    ...lineSummary(coreOutput, 2).map((line) => line.trim()),
  ]);
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
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
  const diagnosticsEndpoint = normalizeDiagnosticsEndpoint(
    options.diagnosticsEndpoint ?? process.env.VOYAVPN_DIAGNOSTICS_ENDPOINT,
    options.mode,
  );
  const workDir = await createWorkDir(options);
  const reporter = new Reporter(options.mode);

  await checkRequiredDocs(reporter);
  await checkNotices(reporter);
  await checkStableEnvironment(reporter, options, cdnBaseUrl, updatesBaseUrl);
  await checkDiagnosticsEndpoint(reporter, options, diagnosticsEndpoint);
  await checkTauriConfig(reporter, options, updatesBaseUrl);
  await scanProductionBlockers(reporter);

  try {
    await checkGeneratedManifests(reporter, options, cdnBaseUrl, updatesBaseUrl, workDir);
  } catch (error) {
    reporter.fail("generated stable metadata", [error.message]);
  }

  reporter.print({ cdnBaseUrl, updatesBaseUrl, diagnosticsEndpoint, workDir });
  if (reporter.hasFailures()) {
    process.exit(1);
  }
}

function isCliEntrypoint() {
  return process.argv[1] ? pathToFileURL(resolve(process.argv[1])).href === import.meta.url : false;
}

if (isCliEntrypoint()) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
