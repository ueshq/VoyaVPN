import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import { capture, truthy } from "../lib/common.mjs";

/** The shell feature that leaves the self-updater out of the binary. */
export const macAppStoreFeature = "mac-app-store";

/**
 * `pnpm build:mac:appstore` sets this. It is a separate switch from
 * `VOYAVPN_MACOS_DISTRIBUTION=app-store` on purpose: `pnpm build:mac:local`
 * signs the same App-Store-shaped appex for local TUN testing and must keep
 * its ordinary configuration.
 */
export function requestedMacAppStoreBuild(env = process.env) {
  return truthy(env.VOYAVPN_MAC_APP_STORE);
}

// CFBundleVersion for a Mac App Store upload: one to three period-separated
// non-negative integers, and each upload of one marketing version must be
// strictly greater than the last.
const bundleVersionPattern = /^\d+(?:\.\d+){0,2}$/u;

/**
 * The build number App Store Connect sees as CFBundleVersion.
 *
 * `VOYAVPN_MACOS_BUILD_NUMBER` wins; otherwise the commit count of HEAD, which
 * only grows on a linear main branch. Without either the upload would reuse
 * the marketing version, and App Store Connect rejects a repeated build.
 */
export function resolveMacAppStoreBuildNumber({ env = process.env, repoRoot, captureCommand = capture } = {}) {
  const explicit = env.VOYAVPN_MACOS_BUILD_NUMBER?.trim();
  if (explicit) {
    if (!bundleVersionPattern.test(explicit)) {
      throw new Error(
        `VOYAVPN_MACOS_BUILD_NUMBER must be one to three period-separated integers (got "${explicit}").`,
      );
    }
    return explicit;
  }

  const result = captureCommand("git", ["rev-list", "--count", "HEAD"], { cwd: repoRoot });
  const count = String(result?.stdout ?? "").trim();
  if (result?.error || result?.status !== 0 || !/^\d+$/u.test(count)) {
    throw new Error(
      "Unable to derive the Mac App Store build number from git; set VOYAVPN_MACOS_BUILD_NUMBER.",
    );
  }
  return count;
}

/**
 * The Tauri config the Mac App Store build merges over `tauri.conf.json`.
 *
 * - Only the `.app` bundle: the signed `.pkg` is made later by
 *   `scripts/native/macos/create-pkg.mjs`, after the PacketTunnel is staged.
 * - macOS 11 is the first release that runs on Apple Silicon, and the store
 *   package is arm64 only.
 * - No updater artifacts: the store delivers updates.
 */
export function macAppStoreOverlay({ buildNumber }) {
  return {
    bundle: {
      targets: ["app"],
      createUpdaterArtifacts: false,
      macOS: {
        minimumSystemVersion: "11.0",
        bundleVersion: buildNumber,
      },
    },
  };
}

export function writeMacAppStoreOverlay({ repoRoot, env = process.env, captureCommand = capture }) {
  const buildNumber = resolveMacAppStoreBuildNumber({ env, repoRoot, captureCommand });
  const overlayPath = resolve(repoRoot, "target", "release-config", "tauri.mac-app-store.generated.json");
  const content = `${JSON.stringify(macAppStoreOverlay({ buildNumber }), null, 2)}\n`;
  if (!existsSync(overlayPath) || readFileSync(overlayPath, "utf8") !== content) {
    mkdirSync(dirname(overlayPath), { recursive: true });
    writeFileSync(overlayPath, content);
  }
  return { overlayPath, buildNumber };
}
