import { existsSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

import { checkedCapture, run, truthy } from "../lib/common.mjs";
import { DEFAULT_SING_BOX_VERSION, singBoxSourcePinStatus } from "../core/sing-box-installer.mjs";

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
 *
 * The commit the ref resolved to is then held against the pin, for every
 * product built from here: the fetch above accepts a tag that was moved
 * upstream, and a ref override names whatever it likes, so without the check
 * the framework that runs inside the tunnel could change with nothing to show
 * for it. A ref with no pinned commit needs the same explicit opt-out the seed
 * archive does.
 */
export function ensureSingBoxSource({
  env = process.env,
  logger = console,
  repoRoot,
  sourceDir = singBoxSourceDir(repoRoot, env),
  ref = singBoxRef(env),
} = {}) {
  const pin = singBoxSourcePinStatus({ env, version: ref });
  if (!pin.pinned && !pin.unpinnedAllowed) {
    throw new Error(pin.reason);
  }

  if (!existsSync(sourceDir)) {
    mkdirSync(dirname(sourceDir), { recursive: true });
    run("git", ["clone", "https://github.com/SagerNet/sing-box.git", sourceDir], { cwd: repoRoot });
  }

  const status = checkedCapture("git", ["status", "--porcelain"], { cwd: sourceDir }).stdout.trim();
  if (status && !truthy(env.VOYAVPN_SING_BOX_ALLOW_DIRTY)) {
    throw new Error(
      `sing-box source checkout has local changes: ${sourceDir}\nSet VOYAVPN_SING_BOX_ALLOW_DIRTY=1 if you intentionally want to build from this checkout.`,
    );
  }

  run("git", ["fetch", "--tags", "--force"], { cwd: sourceDir });
  run("git", ["checkout", ref], { cwd: sourceDir });

  const commit = checkedCapture("git", ["rev-parse", "HEAD"], { cwd: sourceDir }).stdout.trim().toLowerCase();
  if (pin.pinned && commit !== pin.expected) {
    throw new Error(`sing-box ${ref} resolved to ${commit}, not the pinned ${pin.expected}`);
  }
  if (!pin.pinned) {
    logger.warn?.(`  ! unpinned sing-box source: ${ref} at ${commit}`);
  }

  return sourceDir;
}
