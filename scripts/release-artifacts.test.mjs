import { execFile } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { describe, expect, it } from "vitest";

const execFileAsync = promisify(execFile);
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

describe("release artifacts", () => {
  it("skips symbolic links while walking bundle outputs", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-release-artifacts-"));
    const inputDir = join(workDir, "bundle");
    const outputDir = join(workDir, "out");
    const dmgDir = join(inputDir, "dmg");
    const realArtifact = join(dmgDir, "VoyaVPN_0.1.0_x64.dmg");
    const linkedArtifact = join(inputDir, "linked.dmg");

    try {
      await mkdir(dmgDir, { recursive: true });
      await writeFile(realArtifact, "real dmg bytes");
      try {
        await symlink(realArtifact, linkedArtifact);
      } catch (error) {
        if (error && (error.code === "EPERM" || error.code === "ENOSYS")) {
          return;
        }
        throw error;
      }

      await execFileAsync(
        process.execPath,
        [
          "scripts/release-artifacts.mjs",
          "--input",
          inputDir,
          "--output",
          outputDir,
          "--target",
          "darwin-x86_64",
          "--channel",
          "beta",
          "--version",
          "0.1.0",
        ],
        { cwd: repoRoot },
      );

      const manifest = JSON.parse(await readFile(join(outputDir, "artifact-manifest.json"), "utf8"));
      expect(manifest.artifacts).toHaveLength(1);
      expect(manifest.artifacts[0].originalRelativePath).toBe("dmg/VoyaVPN_0.1.0_x64.dmg");
    } finally {
      await rm(workDir, { force: true, recursive: true });
    }
  });
});
