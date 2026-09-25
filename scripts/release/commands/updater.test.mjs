import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { capture, repoRootFromScript } from "../../lib/common.mjs";
import { stableTargets } from "../matrix.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const updatesBaseUrl = "https://updates.voyavpn.dev/stable";
const expectedStableUpdaterTargets = stableTargets
  .map((target) => target.updater)
  .sort((left, right) => left.localeCompare(right));

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

describe("release updater metadata", () => {
  it("verifies every stable updater .sig before writing latest.json", async () => {
    const updaterPublicKey = (await readFile(resolve(repoRoot, "tests/fixtures/release/updater-signing/public.key"), "utf8")).trim();
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-updater-metadata-"));
    const latestPath = join(workDir, "latest.json");

    try {
      const { stdout } = capture(
        process.execPath,
        [
          "scripts/release/cli.mjs",
          "updater",
          "--input",
          "tests/fixtures/release/signed-updater",
          "--out",
          latestPath,
          "--channel",
          "stable",
          "--base-url",
          updatesBaseUrl,
          "--pub-date",
          "2026-06-06T00:00:00.000Z",
        ],
        {
          cwd: repoRoot,
          env: {
            ...process.env,
            VOYAVPN_UPDATER_PUBLIC_KEY: updaterPublicKey,
          },
        },
      );

      const latest = await readJson(latestPath);
      const evidence = await readJson(join(workDir, "latest.evidence.json"));
      expect(Object.keys(latest.platforms).sort()).toEqual(expectedStableUpdaterTargets);
      expect(evidence.validations).toMatchObject({
        updaterPublicKeyApproved: true,
        updaterSignaturesVerified: true,
      });
      expect(Object.values(evidence.platforms).every((entry) => entry.signatureVerified === true)).toBe(true);
      expect(stdout).toContain("Verified updater signatures:");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("emits placeholder metadata for the dry-run channel and refuses it for stable", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-updater-placeholder-"));
    const latestPath = join(workDir, "latest.json");
    const run = (args) =>
      capture(process.execPath, ["scripts/release/cli.mjs", "updater", ...args], { cwd: repoRoot });

    try {
      await run([
        "--input",
        workDir,
        "--out",
        latestPath,
        "--channel",
        "beta",
        "--base-url",
        "https://cdn.voyavpn.test/beta/updater",
        "--target",
        "darwin-x86_64,windows-x86_64",
        "--placeholder-signatures",
      ]);

      const latest = await readJson(latestPath);
      const evidence = await readJson(join(workDir, "latest.evidence.json"));
      expect(Object.keys(latest.platforms).sort()).toEqual(["darwin-x86_64", "windows-x86_64"]);
      expect(latest.platforms["darwin-x86_64"].signature).toBe(
        "VOYAVPN_UPDATER_SIGNATURE_PLACEHOLDER_DARWIN_X86_64",
      );
      expect(latest.platforms["darwin-x86_64"].url).toBe(
        "https://cdn.voyavpn.test/beta/updater/0.1.0/voyavpn-0.1.0-beta-darwin-x86_64-updater.zip",
      );
      expect(Object.values(evidence.platforms).every((entry) => entry.source === "placeholder")).toBe(true);

      const failure = run([
        "--input",
        workDir,
        "--out",
        latestPath,
        "--channel",
        "stable",
        "--base-url",
        updatesBaseUrl,
        "--target",
        "darwin-x86_64",
        "--placeholder-signatures",
      ]);
      expect(failure.status).not.toBe(0);
      expect(`${failure.stderr}${failure.stdout}`).toMatch(/cannot use --placeholder-signatures/);
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("rejects manifests without an explicit updater payload", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-updater-legacy-"));
    const latestPath = join(workDir, "latest.json");

    try {
      const failure = capture(
        process.execPath,
        [
          "scripts/release/cli.mjs",
          "updater",
          "--input",
          "tests/fixtures/release/placeholder-updater",
          "--out",
          latestPath,
          "--channel",
          "beta",
          "--base-url",
          "https://cdn.voyavpn.test/beta/updater",
        ],
        { cwd: repoRoot },
      );
      expect(failure.status).not.toBe(0);
      expect(`${failure.stderr}${failure.stdout}`).toMatch(/No signed updater payload found/);
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });
});
