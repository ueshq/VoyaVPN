import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { describe, expect, it } from "vitest";

const execFileAsync = promisify(execFile);
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const updatesBaseUrl = "https://updates.voyavpn.dev/stable";
const stableTargets = [
  "darwin-aarch64",
  "darwin-x86_64",
  "linux-aarch64",
  "linux-x86_64",
  "windows-aarch64",
  "windows-x86_64",
];

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
          "scripts/release-updater-metadata.mjs",
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
      expect(Object.keys(latest.platforms).sort()).toEqual(stableTargets);
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
});
