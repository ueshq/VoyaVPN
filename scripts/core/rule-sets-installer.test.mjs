import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  ensureRuleSetSeedsForBuild,
  fetchAndStageRuleSets,
  hasStagedRuleSets,
  installRuleSetSeeds,
  RULE_SET_PINS,
  ruleSetSeedDir,
  ruleSetUrl,
  shouldSkipRuleSetInstall,
  verifyStagedRuleSets,
} from "./rule-sets-installer.mjs";

const quiet = { log: () => {} };
const roots = [];

async function tempRoot() {
  const root = await mkdtemp(join(tmpdir(), "voyavpn-rule-sets-"));
  roots.push(root);
  return root;
}

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { force: true, recursive: true })));
});

function fakeRuleSet(tag) {
  return Buffer.from(`SRS fake ${tag}`);
}

function pinsFor(tags) {
  return tags.map((tag) => ({
    commit: "0123456789abcdef0123456789abcdef01234567",
    sha256: createHash("sha256").update(fakeRuleSet(tag)).digest("hex"),
    tag,
  }));
}

function fetchServing(bodies) {
  return vi.fn(async (url) => {
    const body = bodies.get(url);
    return body
      ? { arrayBuffer: async () => body, ok: true, status: 200, statusText: "OK" }
      : { ok: false, status: 404, statusText: "Not Found" };
  });
}

describe("rule-set seeds", () => {
  it("pins the tags the default routing profile generates", () => {
    expect(RULE_SET_PINS.map((pin) => pin.tag).sort()).toEqual(["geoip-cn", "geosite-cn", "geosite-private"]);
    for (const pin of RULE_SET_PINS) {
      expect(pin.commit).toMatch(/^[0-9a-f]{40}$/);
      expect(pin.sha256).toMatch(/^[0-9a-f]{64}$/);
      expect(ruleSetUrl(pin)).toBe(`https://raw.githubusercontent.com/2dust/sing-box-rules/${pin.commit}/${pin.tag}.srs`);
    }
  });

  it("stages every pinned file with a manifest, and verifies them afterwards", async () => {
    const repoRoot = await tempRoot();
    const pins = pinsFor(["geosite-cn", "geoip-cn"]);
    const fetchImpl = fetchServing(new Map(pins.map((pin) => [ruleSetUrl(pin), fakeRuleSet(pin.tag)])));

    await fetchAndStageRuleSets({ fetchImpl, logger: quiet, pins, repoRoot });

    const dir = ruleSetSeedDir(repoRoot);
    expect(readFileSync(join(dir, "geosite-cn.srs"))).toEqual(fakeRuleSet("geosite-cn"));
    expect(hasStagedRuleSets(dir)).toBe(true);
    expect(verifyStagedRuleSets({ pins, repoRoot })).toEqual({ code: "verified", ok: true, reason: null });
    const manifest = JSON.parse(readFileSync(join(dir, "rule-sets.seed.json"), "utf8"));
    expect(manifest.files.map((file) => file.file)).toEqual(["geosite-cn.srs", "geoip-cn.srs"]);

    await writeFile(join(dir, "geoip-cn.srs"), "SRS tampered");
    expect(verifyStagedRuleSets({ pins, repoRoot }).code).toBe("digest-mismatch");
  });

  it("refuses a file that is not a rule set or not the pinned bytes, and replaces nothing", async () => {
    const repoRoot = await tempRoot();
    const [pin] = pinsFor(["geosite-cn"]);

    await expect(fetchAndStageRuleSets({
      fetchImpl: fetchServing(new Map([[ruleSetUrl(pin), Buffer.from("<html>rate limited</html>")]])),
      logger: quiet,
      pins: [pin],
      repoRoot,
    })).rejects.toThrow(/not a sing-box binary rule set/);
    await expect(fetchAndStageRuleSets({
      fetchImpl: fetchServing(new Map([[ruleSetUrl(pin), Buffer.from("SRS other bytes")]])),
      logger: quiet,
      pins: [pin],
      repoRoot,
    })).rejects.toThrow(/SHA-256 mismatch/);
    expect(existsSync(join(ruleSetSeedDir(repoRoot), "geosite-cn.srs"))).toBe(false);
  });

  it("stages for a build only when the pinned files are missing", async () => {
    const repoRoot = await tempRoot();
    const stage = vi.fn(async () => ({ dir: ruleSetSeedDir(repoRoot) }));

    expect(await ensureRuleSetSeedsForBuild({ logger: quiet, repoRoot, stage })).toMatchObject({ status: "staged" });
    expect(stage).toHaveBeenCalledOnce();
  });

  it("skips postinstall on CI unless asked, and when told to", async () => {
    expect(shouldSkipRuleSetInstall({ env: { CI: "true" }, postinstall: true }).skip).toBe(true);
    expect(shouldSkipRuleSetInstall({
      env: { CI: "true", VOYAVPN_FETCH_RULE_SETS_ON_INSTALL: "1" },
      postinstall: true,
    }).skip).toBe(false);
    expect(shouldSkipRuleSetInstall({ env: { VOYAVPN_SKIP_RULE_SETS_POSTINSTALL: "1" } }).skip).toBe(true);
    expect(shouldSkipRuleSetInstall({ env: {} }).skip).toBe(false);

    const stage = vi.fn();
    const result = await installRuleSetSeeds({
      env: { CI: "1" },
      logger: quiet,
      postinstall: true,
      repoRoot: await tempRoot(),
      stage,
    });
    expect(result.status).toBe("skipped");
    expect(stage).not.toHaveBeenCalled();
  });
});
