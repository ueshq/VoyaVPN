import { existsSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

import { checkedCapture, run, truthy } from "../lib/common.mjs";
import { DEFAULT_SING_BOX_VERSION } from "../core/sing-box-installer.mjs";

/**
 * The one sing-box checkout every native build works from.
 *
 * macOS's `Libbox.framework`, iOS's `Libbox.xcframework` and Android's
 * `libbox.aar` are three products of the same source at the same tag. That tag
 * is the app's pinned core version, so the runtime a phone runs is the one the
 * desktop ships — see `docs/release/sing-box-seed-pinning.md` for the bump
 * procedure, and `docs/release/mobile-libbox-pinning.md` for what the mobile
 * artifacts add to it.
 */
function singBoxRef(env = process.env) {
  return env.VOYAVPN_SING_BOX_REF || env.SING_BOX_VERSION || DEFAULT_SING_BOX_VERSION;
}

export function singBoxSourceDir(repoRoot, env = process.env) {
  return resolve(env.VOYAVPN_SING_BOX_SOURCE_DIR || resolve(repoRoot, "target", "native", "sing-box"));
}

/**
 * Clones or updates the checkout and puts it on the pinned ref.
 *
 * A dirty checkout is refused rather than built: the artifact would then be
 * whatever is in the working tree, not the pinned tag, and nothing downstream
 * could tell. `VOYAVPN_SING_BOX_ALLOW_DIRTY=1` is the deliberate override.
 */
export function ensureSingBoxSource({ repoRoot, sourceDir = singBoxSourceDir(repoRoot), ref = singBoxRef() } = {}) {
  if (!existsSync(sourceDir)) {
    mkdirSync(dirname(sourceDir), { recursive: true });
    run("git", ["clone", "https://github.com/SagerNet/sing-box.git", sourceDir], { cwd: repoRoot });
  }

  const status = checkedCapture("git", ["status", "--porcelain"], { cwd: sourceDir }).stdout.trim();
  if (status && !truthy(process.env.VOYAVPN_SING_BOX_ALLOW_DIRTY)) {
    throw new Error(
      `sing-box source checkout has local changes: ${sourceDir}\nSet VOYAVPN_SING_BOX_ALLOW_DIRTY=1 if you intentionally want to build from this checkout.`,
    );
  }

  run("git", ["fetch", "--tags", "--force"], { cwd: sourceDir });
  run("git", ["checkout", ref], { cwd: sourceDir });

  return sourceDir;
}
