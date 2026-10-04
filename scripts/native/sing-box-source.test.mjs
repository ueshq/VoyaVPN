import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { ALLOW_UNPINNED_SING_BOX_ENV, DEFAULT_SING_BOX_VERSION } from "../core/sing-box-installer.mjs";
import { ensureSingBoxSource } from "./sing-box-source.mjs";

const roots = [];

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { force: true, recursive: true });
});

function git(cwd, ...args) {
  return execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
}

/** A checkout whose upstream tags `tag` on a commit of our own making. */
function checkoutWithTag(tag) {
  const root = mkdtempSync(join(tmpdir(), "voyavpn-sing-box-source-test-"));
  roots.push(root);
  const upstream = join(root, "upstream");
  const sourceDir = join(root, "checkout");
  git(root, "init", "--quiet", upstream);
  writeFileSync(join(upstream, "README"), "not sing-box\n");
  git(upstream, "add", "README");
  git(upstream, "-c", "user.name=test", "-c", "user.email=test@example.test", "commit", "--quiet", "-m", "fixture");
  git(upstream, "tag", tag);
  git(root, "clone", "--quiet", upstream, sourceDir);
  return { commit: git(upstream, "rev-parse", "HEAD"), root, sourceDir };
}

describe("the shared sing-box checkout", () => {
  it("refuses a pinned tag that resolves to some other commit", () => {
    const { commit, root, sourceDir } = checkoutWithTag(DEFAULT_SING_BOX_VERSION);

    expect(() =>
      ensureSingBoxSource({ env: {}, ref: DEFAULT_SING_BOX_VERSION, repoRoot: root, sourceDir }),
    ).toThrow(new RegExp(`resolved to ${commit}, not the pinned`));
  });

  it("builds an unpinned ref only when that is asked for", () => {
    const { commit, root, sourceDir } = checkoutWithTag("v0.0.0-fixture");
    const warnings = [];

    expect(() => ensureSingBoxSource({ env: {}, ref: "v0.0.0-fixture", repoRoot: root, sourceDir })).toThrow(
      /has no pinned source commit/,
    );
    expect(
      ensureSingBoxSource({
        env: { [ALLOW_UNPINNED_SING_BOX_ENV]: "1" },
        logger: { warn: (line) => warnings.push(line) },
        ref: "v0.0.0-fixture",
        repoRoot: root,
        sourceDir,
      }),
    ).toBe(sourceDir);
    expect(warnings).toEqual([`  ! unpinned sing-box source: v0.0.0-fixture at ${commit}`]);
  });
});
