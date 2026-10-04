import { existsSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";

import { checkedCapture, run, truthy } from "../lib/common.mjs";
import {
  DEFAULT_SING_BOX_VERSION,
  SING_BOX_SOURCE_REPOSITORY,
  singBoxSourcePinStatus,
} from "../core/sing-box-installer.mjs";

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
/**
 * Installs gomobile from the checkout, the first step of every Libbox build.
 *
 * It names the Go doing the building first, read inside the checkout so it is
 * the toolchain `go.mod` selects there: nothing else records which compiler
 * produced the library that ends up inside the tunnel.
 */
export function installLibboxTools(sourceDir, { logger = console } = {}) {
  logger.log(`Go toolchain: ${checkedCapture("go", ["version"], { cwd: sourceDir }).stdout.trim()}`);
  run("make", ["lib_install"], { cwd: sourceDir });
}

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
    run("git", ["clone", SING_BOX_SOURCE_REPOSITORY, sourceDir], { cwd: repoRoot });
  }

  const status = checkedCapture("git", ["status", "--porcelain"], { cwd: sourceDir }).stdout.trim();
  if (status && !truthy(env.VOYAVPN_SING_BOX_ALLOW_DIRTY)) {
    throw new Error(
      `sing-box source checkout has local changes: ${sourceDir}\nSet VOYAVPN_SING_BOX_ALLOW_DIRTY=1 if you intentionally want to build from this checkout.`,
    );
  }

  const head = () => checkedCapture("git", ["rev-parse", "HEAD"], { cwd: sourceDir }).stdout.trim().toLowerCase();
  // The pinned commit is what is trusted, not the tag. A clean checkout already
  // on it has nothing to learn from the network, so every build after the
  // first skips the fetch.
  if (!(pin.pinned && !status && head() === pin.expected)) {
    run("git", ["fetch", "--tags", "--force"], { cwd: sourceDir });
    run("git", ["checkout", ref], { cwd: sourceDir });
  }

  const commit = head();
  if (pin.pinned && commit !== pin.expected) {
    throw new Error(`sing-box ${ref} resolved to ${commit}, not the pinned ${pin.expected}`);
  }
  if (!pin.pinned) {
    logger.warn?.(`  ! unpinned sing-box source: ${ref} at ${commit}`);
  }

  return sourceDir;
}
