import { stat } from "node:fs/promises";
import { basename, dirname, join, relative } from "node:path";

import { walkFiles } from "../lib/fs.mjs";

// Single source of truth for the release gate's shared validators. Every
// release command imports from here: when host, placeholder, digest or updater
// payload rules were copied per command they drifted, and a stable gate that
// means something different in each file is not a gate.

const placeholderPattern =
  /placeholder|replace_before_release|replace-before-release|changeme|\btodo\b|\btbd\b|voyavpn\.example/i;

export function missingExpectedValues(expected, present) {
  return expected.filter((value) => !present.has(value));
}

export function isSha256Hex(value) {
  return /^[a-f0-9]{64}$/i.test(value ?? "");
}

export function isPositiveByteSize(value) {
  return Number.isInteger(value) && value > 0;
}

export function isUrlDerivedFromBase(url, baseUrl) {
  return url?.startsWith(`${baseUrl}/`);
}

export function uniqueSorted(values) {
  return [...new Set(values.filter(Boolean))].sort((left, right) => left.localeCompare(right));
}

export function isStableChannel(channel) {
  return String(channel ?? "").trim().toLowerCase() === "stable";
}

/** Detects the placeholder markers the credential-free configs and fixtures use. */
export function placeholderText(value) {
  return !value || placeholderPattern.test(String(value));
}

/**
 * Names why a hostname may not appear in stable release metadata, or null when
 * it is acceptable. `allowTestHosts` keeps localhost/.test usable for the local
 * verification server; everything else is rejected in every caller.
 */
export function forbiddenHostReason(hostname, { allowTestHosts = false } = {}) {
  const host = String(hostname ?? "").toLowerCase();
  const isLocalHost = host === "localhost" || host === "127.0.0.1" || host === "::1" || host.endsWith(".test");

  if (allowTestHosts && isLocalHost) {
    return null;
  }
  if (host.includes("example")) {
    return "example host";
  }
  if (
    host === "github.com" ||
    host.endsWith(".github.com") ||
    host.includes("githubusercontent.com") ||
    host === "github.io" ||
    host.endsWith(".github.io")
  ) {
    return "GitHub host";
  }
  if (isLocalHost) {
    return "local or test host";
  }
  if (host.includes("placeholder")) {
    return "placeholder host";
  }
  return null;
}

/**
 * Normalizes a release base URL: protocol policy, no embedded credentials, no
 * query string or fragment, no trailing slash, and the shared forbidden-host
 * rule. Callers pass `label` so each command keeps its own operator-facing
 * wording, and decide whether an absent value is an error.
 */
export function normalizeReleaseUrl(
  value,
  { allowHttp = false, allowTestHosts = false, checkHost = true, label = "base URL" } = {},
) {
  const text = String(value ?? "").trim();
  let parsed;
  try {
    parsed = new URL(text);
  } catch {
    throw new Error(`${label} is not a valid URL: ${text}`);
  }

  const protocolAllowed = allowHttp
    ? parsed.protocol === "https:" || parsed.protocol === "http:"
    : parsed.protocol === "https:";
  if (!protocolAllowed) {
    throw new Error(`${label} must use ${allowHttp ? "http or https" : "https"}: ${text}`);
  }
  if (parsed.username || parsed.password) {
    throw new Error(`${label} must not include credentials: ${text}`);
  }
  if (checkHost) {
    const reason = forbiddenHostReason(parsed.hostname, { allowTestHosts });
    if (reason) {
      throw new Error(`${label} must not use ${reason}: ${text}`);
    }
  }

  parsed.hash = "";
  parsed.search = "";
  return parsed.toString().replace(/\/+$/g, "");
}

/**
 * The base URL a metadata command publishes under. Callers keep their own
 * operator-facing wording (`missing`, `label`); the dry-run lane generates
 * "stable" metadata against the placeholder `.test` CDN, so local/test hosts
 * stay allowed here and readiness is the gate that rejects them for a real
 * stable run. `rejectPlaceholderPath` also refuses a stable URL whose *path*
 * says placeholder, which the host rule alone would let through.
 */
export function normalizeChannelBaseUrl(
  baseUrl,
  channel,
  { missing, label, allowHttp = true, rejectPlaceholderPath = false },
) {
  const value = String(baseUrl ?? "").trim();
  if (!value) {
    throw new Error(missing);
  }

  const normalized = normalizeReleaseUrl(value, {
    allowHttp,
    allowTestHosts: true,
    checkHost: isStableChannel(channel),
    label,
  });

  if (rejectPlaceholderPath && isStableChannel(channel) && normalized.toLowerCase().includes("placeholder")) {
    throw new Error(`${label} must not use example, GitHub, or placeholder hosts: ${value}`);
  }

  return normalized;
}

/** Whether a document, serialized whole, carries placeholder or GitHub content. */
export function forbiddenSerializedContent(value) {
  const text = JSON.stringify(value).toLowerCase();
  return placeholderText(text) || text.includes("github.com");
}

/** Fails with `message: <missing>` unless every expected target is present. */
export function assertStableTargetMatrix(present, expected, message) {
  const missing = missingExpectedValues(expected, new Set(present));
  if (missing.length > 0) {
    throw new Error(`${message}: ${missing.join(", ")}`);
  }
}

/** The stable release index: no forbidden content, CDN-derived URLs, full target matrix. */
export function assertStableIndex(index, baseUrl, { present, expected }) {
  if (forbiddenSerializedContent(index)) {
    throw new Error("Stable release index contains forbidden placeholder or GitHub content");
  }

  for (const artifact of index.artifacts) {
    if (!isUrlDerivedFromBase(artifact.url, baseUrl)) {
      throw new Error(`Artifact URL is not derived from CDN base URL: ${artifact.url}`);
    }
  }

  assertStableTargetMatrix(present, expected, "Stable release index is missing first-stable target(s)");
}

/** Stable `latest.json` and its evidence: every platform is a verified signed artifact under the base URL. */
export function assertStableDocuments(latest, evidenceDocument, baseUrl) {
  if (forbiddenSerializedContent(latest)) {
    throw new Error("Stable updater latest.json contains forbidden placeholder or GitHub content");
  }

  for (const [target, platform] of Object.entries(latest.platforms)) {
    if (!isUrlDerivedFromBase(platform.url, baseUrl)) {
      throw new Error(`Stable updater URL for ${target} is not derived from base URL: ${platform.url}`);
    }
    const evidence = evidenceDocument.platforms[target];
    if (evidence?.source !== "signed-artifact") {
      throw new Error(`Stable updater evidence for ${target} does not map to a signed artifact`);
    }
    if (evidence.signatureVerified !== true) {
      throw new Error(`Stable updater signature for ${target} was not verified with the approved updater public key`);
    }
  }
}

/**
 * The per-target evidence block the release index and the updater metadata
 * both record: `{ name, originalName, bytes, sha256 }` entries in, counts,
 * sorted names and checksums out. Entries without a digest (dry-run
 * placeholders) are counted and named but carry no checksum.
 */
export function artifactEvidence(artifacts) {
  return {
    artifactCount: artifacts.length,
    artifactNames: uniqueSorted(artifacts.map((artifact) => artifact.name)),
    sourceArtifactNames: uniqueSorted(artifacts.map((artifact) => artifact.originalName)),
    checksums: artifacts
      .filter((artifact) => artifact.sha256)
      .map((artifact) => ({
        name: artifact.name,
        sourceArtifactName: artifact.originalName,
        bytes: artifact.bytes,
        sha256: artifact.sha256,
      })),
  };
}

export function requiredString(value, field, context) {
  if (!value || typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${context} is missing ${field}`);
  }
  return value.trim();
}

/** `requiredString` for fields a stable manifest must never carry a placeholder in. */
export function requiredNonPlaceholderString(value, field, context) {
  const trimmed = requiredString(value, field, context);
  if (/placeholder/i.test(trimmed)) {
    throw new Error(`${context} contains placeholder ${field}`);
  }
  return trimmed;
}

export function requiredSha256(value, context, { rejectRepeatedDigits = false } = {}) {
  const hash = requiredString(value, "sha256", context).toLowerCase();
  if (!isSha256Hex(hash)) {
    throw new Error(`${context} has invalid sha256: ${value}`);
  }
  if (rejectRepeatedDigits && /^([a-f0-9])\1{63}$/.test(hash)) {
    throw new Error(`${context} has placeholder-like sha256: ${value}`);
  }
  return hash;
}

export function requiredBytes(value, context) {
  if (!isPositiveByteSize(value)) {
    throw new Error(`${context} has invalid bytes: ${value}`);
  }
  return value;
}

export function joinUrl(baseUrl, ...parts) {
  const segments = parts
    .flatMap((part) => String(part).split("/"))
    .filter((segment) => segment.length > 0)
    .map((segment) => encodeURIComponent(segment));

  return [String(baseUrl).replace(/\/+$/g, ""), ...segments].join("/");
}

/** Normalizes and rejects an artifact path that would escape the manifest directory. */
export function safeArtifactPath(artifact, context) {
  const value = artifact?.path ?? artifact?.name;
  if (!value || typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${context} is missing path or name`);
  }

  const normalized = value.trim().replaceAll("\\", "/");
  if (normalized.startsWith("/") || normalized.split("/").some((segment) => segment === "..")) {
    throw new Error(`${context} has an unsafe artifact path: ${value}`);
  }

  return normalized;
}

function isSignatureArtifact(artifact) {
  return (
    artifact?.kind === "signature" || String(artifact?.name ?? "").toLowerCase().endsWith(".sig")
  );
}

/**
 * Picks the single artifact the in-place Tauri 2 updater serves for a target.
 *
 * `pnpm release -- artifacts` marks it explicitly (`updaterPayload: true`),
 * because with `createUpdaterArtifacts: true` the signed payload is the
 * installer itself (NSIS `-setup.exe`, `.AppImage`, `.app.tar.gz`) and several
 * artifacts of a target can carry a sibling `.sig`.
 */
export function selectUpdaterPayload(artifacts) {
  const candidates = (artifacts ?? []).filter((artifact) => !isSignatureArtifact(artifact));
  const marked = candidates.filter((artifact) => artifact.updaterPayload === true);
  if (marked.length > 1) {
    throw new Error(
      `artifact manifest marks ${marked.length} updater payloads: ${marked.map((artifact) => artifact.name).join(", ")}`,
    );
  }
  return marked[0] ?? null;
}

export function findSignatureArtifact(payload, artifacts) {
  return (artifacts ?? []).find((artifact) => {
    if (artifact.kind !== "signature") {
      return false;
    }

    return (
      artifact.originalRelativePath === `${payload.originalRelativePath}.sig` ||
      artifact.originalName === `${payload.originalName}.sig` ||
      artifact.path === `${payload.path}.sig` ||
      artifact.name === `${payload.name}.sig`
    );
  });
}

export function defaultEvidencePath(outputPath) {
  const name = basename(outputPath);
  const dot = name.lastIndexOf(".");
  const evidenceName = dot === -1 ? `${name}.evidence.json` : `${name.slice(0, dot)}.evidence.json`;
  return join(dirname(outputPath), evidenceName);
}

export function sourceInputEvidence(inputPath, repoRoot, productionKind = "workflow-artifact") {
  const relativePath = relative(repoRoot, inputPath).replaceAll("\\", "/") || ".";
  const isFixture = relativePath === "tests/fixtures" || relativePath.startsWith("tests/fixtures/");
  return {
    path: inputPath,
    relativePath,
    kind: isFixture ? "fixture" : productionKind,
    nonPublishableFixture: isFixture,
  };
}

export async function walkArtifactManifests(root) {
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

  return walkFiles(root, { match: (name) => name === "artifact-manifest.json" }).then((paths) =>
    [...paths].sort((left, right) => left.localeCompare(right)),
  );
}
