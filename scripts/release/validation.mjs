import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { readdir, stat } from "node:fs/promises";
import { basename, dirname, join, relative, resolve } from "node:path";

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

export function isForbiddenStableHost(hostname, options = {}) {
  return forbiddenHostReason(hostname, options) !== null;
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

  const manifests = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    // Symlinks are skipped so a link planted in an artifact directory cannot
    // pull a manifest from outside the downloaded release tree.
    if (entry.isSymbolicLink()) {
      continue;
    }

    const path = resolve(root, entry.name);
    if (entry.isDirectory()) {
      manifests.push(...(await walkArtifactManifests(path)));
    } else if (entry.isFile() && entry.name === "artifact-manifest.json") {
      manifests.push(path);
    }
  }
  return manifests.sort((left, right) => left.localeCompare(right));
}

export function sha256Text(value) {
  return createHash("sha256").update(value).digest("hex");
}

export async function sha256File(path) {
  const hash = createHash("sha256");
  await new Promise((resolvePromise, rejectPromise) => {
    const stream = createReadStream(path);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("error", rejectPromise);
    stream.on("end", resolvePromise);
  });
  return hash.digest("hex");
}
