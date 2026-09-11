import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { repoRootFromScript } from "../../lib/common.mjs";
import { main } from "./core-assets.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const temporaryDirectories = [];

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((path) => rm(path, { force: true, recursive: true })));
});

describe("core asset manifest", () => {
  it("keeps the current empty stable manifest and writes matching evidence", async () => {
    const outputDir = await mkdtemp(join(tmpdir(), "voyavpn-core-assets-"));
    temporaryDirectories.push(outputDir);
    const output = join(outputDir, "core-assets.json");

    await main([
      "--fixture",
      resolve(repoRoot, "tests/fixtures/release/core-assets.json"),
      "--out",
      output,
      "--base-url",
      "https://cdn.voyavpn.dev/stable",
      "--channel",
      "stable",
    ]);

    const manifest = JSON.parse(await readFile(output, "utf8"));
    const evidence = JSON.parse(await readFile(join(outputDir, "core-assets.evidence.json"), "utf8"));
    expect(manifest.assets).toEqual([]);
    expect(evidence.assetCount).toBe(0);
    expect(evidence.validations.githubUrlsOnlyInUpstreamReferences).toBe(true);
  });

  it.each(["stable", "beta"])("rejects standalone core downloads for %s", async (channel) => {
    const outputDir = await mkdtemp(join(tmpdir(), "voyavpn-core-assets-rejected-"));
    temporaryDirectories.push(outputDir);
    const fixture = join(outputDir, "source.json");
    const output = join(outputDir, "core-assets.json");
    await writeFile(fixture, JSON.stringify({ assets: [{ coreType: "sing_box" }] }));

    await expect(main([
      "--fixture", fixture, "--out", output, "--channel", channel,
      "--base-url", "https://cdn.voyavpn.dev/stable",
    ])).rejects.toThrow(/not supported/);
    await expect(readFile(output)).rejects.toMatchObject({ code: "ENOENT" });
  });

  it("fails closed for a GitHub stable CDN base", async () => {
    await expect(main([
      "--fixture",
      resolve(repoRoot, "tests/fixtures/release/core-assets.json"),
      "--out",
      resolve(repoRoot, "target/test-core-assets.json"),
      "--base-url",
      "https://github.com/voyavpn/releases",
      "--channel",
      "stable",
    ])).rejects.toThrow(/GitHub|production/i);
  });
});
