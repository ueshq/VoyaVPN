import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { describe, expect, it } from "vitest";
import { stableTargets } from "../matrix.mjs";

const execFileAsync = promisify(execFile);
const repoRoot = resolve(fileURLToPath(new URL("../../..", import.meta.url)));
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
      const { stdout } = await execFileAsync(
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
            TAURI_UPDATER_PUBLIC_KEY: updaterPublicKey,
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
      execFileAsync(process.execPath, ["scripts/release/cli.mjs", "updater", ...args], { cwd: repoRoot });

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

      await expect(
        run([
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
        ]),
      ).rejects.toThrow(/cannot use --placeholder-signatures/);
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });

  it("resolves the legacy kind:updater payload when a manifest predates updaterPayload", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-updater-legacy-"));
    const latestPath = join(workDir, "latest.json");

    try {
      await execFileAsync(
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

      const latest = await readJson(latestPath);
      expect(latest.platforms["windows-x86_64"].url).toBe(
        "https://cdn.voyavpn.test/beta/updater/voyavpn-0.1.0-stable-windows-x86-64-updater.zip",
      );
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });
});
