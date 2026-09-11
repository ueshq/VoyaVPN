import { readFile } from "node:fs/promises";
import { resolveRepoPath } from "./inputs.mjs";
import { lineSummary } from "./reporter.mjs";

export const blockerScanFiles = [
  "apps/desktop/src-tauri/tauri.conf.json",
  ".github/workflows/release.yml",
  "docs/release/packaging.md",
  "docs/release/ci-secrets.md",
  "docs/release/signing-notarization.md",
  "docs/release/os-smoke-matrix.md",
  "docs/release/rollback.md",
  "docs/release/runbook.md",
  "docs/release/THIRD_PARTY_NOTICES.md",
  "docs/verification/stable-release-gate.md",
  "crates/voya-net/src/lib.rs",
  "crates/voya-net/src/download.rs",
  "crates/voya-net/src/subscription.rs",
];

const urlTextRegex = /\bhttps?:\/\/[^\s"'`<>]+/gi;
const githubDownloadPathRegex = /\/releases\/(?:latest\/)?download\/|\/latest\/download\//i;
const exampleTextRegex = /\bhttps?:\/\/[^\s"'`<>)]*voyavpn\.example[^\s"'`<>)]*|\bvoyavpn\.example\b/i;
const updaterPlaceholderRegex = /\bVOYAVPN_UPDATER_(?:PUBLIC_KEY|SIGNATURE)_PLACEHOLDER[A-Z0-9_]*\b/i;
const productionFieldRegex =
  /(?:^|[\s{[,("'])(?:url|urls|downloadUrl|download_url|download-url|cdnUrl|cdn_url|assetUrl|asset_url|artifactUrl|artifact_url|payloadUrl|payload_url|installerUrl|installer_url|manualDownloadUrl|manual_download_url|releaseIndexUrl|release_index_url|latestJsonUrl|latest_json_url|baseUrl|base_url|updatesBaseUrl|updates_base_url|endpoint|endpoints|downloadUrlTemplate|download_url_template|urlTemplate|url_template)["']?\s*[:=]|(?:^|\s)VOYAVPN_(?:CDN_BASE_URL|UPDATES_BASE_URL)\s*=/i;
const productionCliUrlRegex = /--(?:cdn-base-url|updates-base-url|base-url)\s+\S+/i;
const productionTemplateContextRegex =
  /\b(?:ReleasePackage|AssetTemplates|downloadTemplates|download_templates|downloadUrlTemplate|download_url_template|urlTemplate|url_template|templates)\b/i;
const sourceEvidenceContextRegex =
  /\b(?:upstreamUrl|upstream_url|upstream|sourceUrl|source_url|SOURCE_URL|sourceBundleDir|sourceManifests|source reference|source evidence|release_api_url|release_url|html_url|repository|homepage|licenseUrl|license_url|UpstreamReleaseEvidence|UpstreamAssetTemplates|asset_templates)\b/i;
const guardOrDefensiveContextRegex =
  /\b(?:forbidden|rejects?|allowed only|must not|should not|contains|includes|placeholder\.test|throw new Error|expect_err|assert|no `?voyavpn\.example|no .*github|validation fails|fails when)\b/i;

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

export async function scanProductionBlockers(reporter) {
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
