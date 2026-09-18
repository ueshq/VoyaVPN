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

  it("bounds every job with an explicit timeout", () => {
    const unbounded = [];
    for (const file of workflowFiles()) {
      for (const [job, body] of workflowJobs(readWorkflow(file))) {
        // A job that only calls a reusable workflow inherits that workflow's
        // per-job timeouts and cannot declare one of its own.
        const callsReusableWorkflow = body.some((line) => /^ {4}uses:/.test(line));
        if (callsReusableWorkflow || body.some((line) => /^ {4}timeout-minutes:\s*\d+/.test(line))) {
          continue;
        }
        unbounded.push(`${file}:${job}`);
      }
    }

    expect(unbounded).toEqual([]);
  });

  it("scopes release signing secrets to the runner that can use them", () => {
    const release = readWorkflow("release.yml");
    const buildStep = release.slice(
      release.indexOf("      - name: Build Tauri package"),
      release.indexOf("      - name: Normalize artifacts and write checksums"),
    );

    expect(buildStep).not.toBe("");
    // Apple material never reaches the Windows and Linux package runners.
    for (const secret of ["APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID"]) {
      expect(buildStep, secret).toContain(`${secret}: \${{ runner.os == 'macOS' && secrets.${secret} || '' }}`);
    }
    // Nothing on the pnpm tauri:build path consumes the Windows certificate.
    expect(buildStep).not.toContain("WINDOWS_CERTIFICATE_BASE64");
    expect(buildStep).not.toContain("WINDOWS_CERTIFICATE_PASSWORD");
  });

  it("gives presence-check-only steps booleans instead of the secrets themselves", () => {
    const release = readWorkflow("release.yml");
    const presenceSteps = [
      release.slice(
        release.indexOf("      - name: Validate stable release prerequisites"),
        release.indexOf("  package:"),
      ),
      release.slice(release.indexOf("      - name: Validate final stable readiness")),
    ];

    for (const step of presenceSteps) {
      expect(step).not.toBe("");
      for (const secret of ["APPLE_CERTIFICATE", "APPLE_ID", "APPLE_PASSWORD", "WINDOWS_CERTIFICATE_BASE64"]) {
        expect(step, secret).toContain(`HAS_${secret}: \${{ secrets.${secret} != '' }}`);
        expect(step, secret).not.toMatch(new RegExp(`^\\s+${secret}: `, "m"));
      }
    }
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

  // Without a build cache every job recompiled the whole Tauri workspace
  // (webkit2gtk, tao/wry, sqlx, tokio, specta) from scratch on every run.
  it("caches the cargo build directory in every CI job that compiles the workspace", () => {
    const compilesWorkspace = /cargo (?:check|clippy|test|build)\b|tauri:build|check:rust:|check:desktop:smoke/;
    const uncached = [];

    for (const [job, body] of workflowJobs(readWorkflow("ci.yml"))) {
      const text = body.join("\n");
      if (!compilesWorkspace.test(text)) {
        continue;
      }
      if (!/uses:\s*Swatinem\/rust-cache@[0-9a-f]{40}\b/.test(text)) {
        uncached.push(`ci.yml:${job}`);
      }
    }

    expect(uncached).toEqual([]);
  });

  // `cargo install` builds these tools from source; the cache turns a pinned
  // version into a no-op. Swatinem/rust-cache deliberately skips ~/.cargo/bin,
  // so each tool needs its own actions/cache entry.
  it("caches the binary of every cargo-installed tool", () => {
    const missing = [];

    for (const file of workflowFiles()) {
      for (const [job, body] of workflowJobs(readWorkflow(file))) {
        const text = body.join("\n");
        // Comments mention `cargo install` too; only real steps count.
        const commands = body.filter((line) => !line.trim().startsWith("#")).join("\n");
        for (const match of commands.matchAll(/cargo install (\S+)/gu)) {
          const tool = match[1];
          if (!text.includes(`~/.cargo/bin/${tool}`) || !/uses:\s*actions\/cache@[0-9a-f]{40}\b/.test(text)) {
            missing.push(`${file}:${job}:${tool}`);
          }
        }
      }
    }

    expect(missing).toEqual([]);
  });

  it("documents why the release package matrix opts out of the Rust build cache", () => {
    const packageJob = workflowJobs(readWorkflow("release.yml")).get("package");

    expect(packageJob).toBeDefined();
    const text = packageJob.join("\n");
    expect(text).not.toContain("Swatinem/rust-cache@");
    expect(text).toContain("deliberately has no Swatinem/rust-cache step");
  });

  it("lints Rust on every platform-check runner so OS-gated code is covered", () => {
    const jobs = workflowJobs(readWorkflow("ci.yml"));
    const platformCheck = jobs.get("platform-check");
    expect(platformCheck).toBeDefined();
    const body = platformCheck.join("\n");
    expect(body).toMatch(/cargo clippy --workspace --all-targets --locked -- -D warnings/);
    expect(body).toMatch(/macos-15/);
    expect(body).toMatch(/windows-2025/);

    // Linux clippy runs once, in baseline-rust; a Linux leg here linted the
    // same target a second time.
    expect(body).not.toMatch(/ubuntu-/);
    expect(jobs.get("baseline-rust")?.join("\n")).toContain("pnpm run check:rust:clippy");
  });

  // A cache hit only skips the browser download; the apt libraries Chromium
  // needs are not in ~/.cache/ms-playwright and must still be installed.
  it("installs Chromium's system libraries whether or not the browser cache hits", () => {
    for (const [job, body] of workflowJobs(readWorkflow("ci.yml"))) {
      const text = body.join("\n");
      if (!/playwright install\b/.test(text)) {
        continue;
      }
      expect(text, job).toMatch(/path: ~\/\.cache\/ms-playwright/);
      expect(text, job).toContain("playwright install --with-deps chromium");
      expect(text, job).toContain("playwright install-deps chromium");
    }
  });
});
