import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(fileURLToPath(new URL("../..", import.meta.url)));
const workflowDir = resolve(repoRoot, ".github", "workflows");
const jobHeaderPattern = /^ {2}([A-Za-z0-9_-]+):\s*$/;
const jobBodyPattern = /^ {4}(?:steps|uses):/;
const pnpmInvocationPattern = /(?:^|[\s"'(&|;])pnpm(?:\s|$)/;
const pnpmSetupPattern = /uses:\s*pnpm\/action-setup@[0-9a-f]{40}\b/;
const actionPinPattern = /^\s*(?:-\s*)?uses:\s*([^\s@]+)@([^\s#]+)/;

function workflowFiles() {
  return readdirSync(workflowDir)
    .filter((name) => name.endsWith(".yml") || name.endsWith(".yaml"))
    .sort();
}

function readWorkflow(name) {
  return readFileSync(resolve(workflowDir, name), "utf8");
}

/**
 * Splits the top-level `jobs:` mapping into per-job line blocks. This is a
 * deliberately small scanner instead of a YAML parser so the quality gates keep
 * running on Node builtins only.
 */
function workflowJobs(text) {
  const lines = text.split(/\r?\n/);
  const jobsIndex = lines.indexOf("jobs:");
  const jobs = new Map();
  if (jobsIndex < 0) {
    return jobs;
  }

  let current = null;
  for (const line of lines.slice(jobsIndex + 1)) {
    const header = jobHeaderPattern.exec(line);
    if (header) {
      current = header[1];
      jobs.set(current, []);
      continue;
    }
    if (current) {
      jobs.get(current).push(line);
    }
  }

  // Drop anything that does not look like a job definition, so an unexpected
  // two-space line inside a shell block cannot invent a phantom job.
  for (const [name, body] of jobs) {
    if (!body.some((line) => jobBodyPattern.test(line))) {
      jobs.delete(name);
    }
  }
  return jobs;
}

function invokesPnpm(body) {
  return body.some((line) => {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      return false;
    }
    if (/uses:|cache:\s*pnpm|PNPM_VERSION/.test(trimmed)) {
      return false;
    }
    return pnpmInvocationPattern.test(trimmed);
  });
}

function installsPnpm(body) {
  return body.some((line) => pnpmSetupPattern.test(line));
}

function jobsInvokingPnpm(name) {
  return [...workflowJobs(readWorkflow(name))]
    .filter(([, body]) => invokesPnpm(body))
    .map(([job]) => job);
}

describe("GitHub Actions workflows", () => {
  it("discovers the jobs of every workflow file", () => {
    const files = workflowFiles();
    expect(files).toEqual(expect.arrayContaining(["ci.yml", "release.yml"]));
    for (const file of files) {
      expect(workflowJobs(readWorkflow(file)).size).toBeGreaterThan(0);
    }
  });

  it("installs pnpm in every job that runs pnpm", () => {
    const missing = [];
    for (const file of workflowFiles()) {
      for (const [job, body] of workflowJobs(readWorkflow(file))) {
        if (invokesPnpm(body) && !installsPnpm(body)) {
          missing.push(`${file}:${job}`);
        }
      }
    }

    expect(missing).toEqual([]);
  });

  it("covers the release metadata jobs that run the release CLI through pnpm", () => {
    expect(jobsInvokingPnpm("release.yml")).toEqual(
      expect.arrayContaining(["core-staging-metadata", "final-stable-readiness"]),
    );
  });

  it("pins every third-party action to a full commit SHA", () => {
    const unpinned = [];
    for (const file of workflowFiles()) {
      const text = readWorkflow(file);
      for (const line of text.split(/\r?\n/)) {
        const match = actionPinPattern.exec(line);
        if (!match || match[1].startsWith("./")) {
          continue;
        }
        if (!/^[0-9a-f]{40}$/.test(match[2])) {
          unpinned.push(`${file}: ${match[1]}@${match[2]}`);
        }
      }
    }

    expect(unpinned).toEqual([]);
  });

  it("lints Rust on every platform-check runner so OS-gated code is covered", () => {
    const platformCheck = workflowJobs(readWorkflow("ci.yml")).get("platform-check");
    expect(platformCheck).toBeDefined();
    const body = platformCheck.join("\n");
    expect(body).toMatch(/cargo clippy --workspace --all-targets --locked -- -D warnings/);
    expect(body).toMatch(/macos-15/);
    expect(body).toMatch(/windows-2025/);
  });
});
