import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { environmentValue, repoRootFromScript } from "../../../lib/common.mjs";
import { readJsonAsync } from "../../../lib/fs.mjs";
import { normalizeReleaseUrl } from "../../validation.mjs";
export const repoRoot = repoRootFromScript(import.meta.url);
export const defaultTauriConfig = "apps/desktop/src-tauri/tauri.conf.json";
export const generatedStableUpdaterConfig = "target/release-config/tauri.updater.stable.generated.json";

export function isDryRun(options) {
  return options.mode === "dry-run";
}

export function displayPath(path) {
  return relative(repoRoot, path).replaceAll("\\", "/") || ".";
}

export function resolveRepoPath(path) {
  return resolve(repoRoot, path);
}

/** A stable input path: the CLI option, else the environment, else the default. */
export function stableInputPath(explicit, envName, fallback) {
  return explicit ?? (environmentValue(process.env, envName) || fallback);
}

export function assertStableEvidencePath(options, label, path) {
  if (isDryRun(options)) {
    return;
  }

  const normalized = relative(repoRoot, resolve(repoRoot, path)).replaceAll("\\", "/");
  if (normalized === "tests/fixtures" || normalized.startsWith("tests/fixtures/")) {
    throw new Error(`${label} must not use tests/fixtures in stable mode: ${normalized}`);
  }
}

export async function createWorkDir(options) {
  if (options.workDir) {
    return resolve(repoRoot, options.workDir);
  }
  return mkdtemp(join(tmpdir(), "voyavpn-readiness-"));
}

export function normalizeUrl(value, label, mode, { defaultDryRunUrl = null, requireHttps = false } = {}) {
  const resolvedValue = (value ?? (mode === "dry-run" ? defaultDryRunUrl : null) ?? "").trim();
  if (!resolvedValue) {
    throw new Error(`${label} is required`);
  }

  // Dry-run readiness runs against the local `.test` placeholder CDN, so
  // local/test hosts are rejected only in stable mode.
  return normalizeReleaseUrl(resolvedValue, {
    allowHttp: !requireHttps,
    allowTestHosts: mode !== "stable",
    label,
  });
}

export { readJsonAsync };

export function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}
