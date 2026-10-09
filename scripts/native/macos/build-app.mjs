import { existsSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import {
  captureSpawned,
  isCliEntrypoint,
  repoRootFromScript,
  requireDarwin,
  run,
  runCli,
  sleepSync,
  truthy,
} from "../../lib/common.mjs";
import { readJson } from "../../lib/fs.mjs";
import { appBundleIdentifier, packetTunnelBundleIdentifier, resolveDmgPath } from "./tunnel-layout.mjs";
import { requestedMacAppStoreBuild } from "../../tauri/mac-app-store-config.mjs";
import { defaultIsProcessRunning, prepareVoyaForLocalBuild } from "./local-runtime.mjs";
import {
  defaultProvisioningProfileDir,
  distributionProfileLabel,
  formatProfileSelectionError,
  installedProvisioningProfileDir,
  localProvisioningUdid,
  plistBuddy,
  resolveProfileFromEnv,
  resolveSigningIdentity,
} from "./provisioning.mjs";
import { requireGoToolchain } from "../../core/sing-box-source-seed.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const packageJson = readJson(resolve(repoRoot, "package.json"));
const appBundle = resolve(repoRoot, "target", "release", "bundle", "macos", "VoyaVPN.app");
const appContents = resolve(appBundle, "Contents");
const dmgDir = resolve(repoRoot, "target", "release", "bundle", "dmg");
const installedAppBundle = "/Applications/VoyaVPN.app";

function commandOptions(env = process.env) {
  return { cwd: repoRoot, env };
}

function commandStatus(program, args, env = process.env) {
  const result = captureSpawned(program, args, {
    ...commandOptions(env),
    stdio: "inherit",
  });
  return result.status ?? 1;
}

function withoutEnv(env, names) {
  const next = { ...env };
  for (const name of names) {
    delete next[name];
  }
  return next;
}

function signingIdentity(pattern, label) {
  const explicit = process.env.VOYAVPN_CODESIGN_IDENTITY?.trim();
  try {
    return resolveSigningIdentity(explicit || pattern, label).sha1;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`${message}\nSet VOYAVPN_CODESIGN_IDENTITY and retry.`, { cause: error });
  }
}

function developerIdIdentity() {
  return signingIdentity(/Developer ID Application/, "Developer ID Application");
}

function localAppExtensionIdentity() {
  return signingIdentity(/Apple Development:|Mac Developer:/, "Apple Development or Mac Developer");
}

function appStoreIdentity() {
  return signingIdentity(
    /3rd Party Mac Developer Application:|Apple Distribution:/,
    "3rd Party Mac Developer Application or Apple Distribution",
  );
}

/**
 * Three lanes share this script:
 * - `developer-id` (`vp run build mac`): notarized DMG with a System Extension.
 * - `local` (`vp run build mac local`): App-Store-shaped appex signed with an
 *   Apple Development identity, installed into /Applications for TUN testing.
 * - `app-store` (`vp run build mac appstore`): the signed `.pkg` uploaded to App
 *   Store Connect with Transporter. No DMG, no notarization, no install.
 */
function buildLane() {
  if (requestedMacAppStoreBuild()) {
    return "app-store";
  }
  return skipNotarization() ? "local" : "developer-id";
}

function skipNotarization() {
  return truthy(process.env.VOYAVPN_SKIP_NOTARIZATION);
}

function requireNotaryCredentials(lane) {
  if (lane !== "developer-id") {
    return;
  }
  if (process.env.VOYAVPN_NOTARY_KEYCHAIN_PROFILE?.trim()) {
    return;
  }
  if (
    process.env.VOYAVPN_NOTARY_APPLE_ID?.trim() &&
    process.env.VOYAVPN_NOTARY_TEAM_ID?.trim() &&
    process.env.VOYAVPN_NOTARY_PASSWORD?.trim()
  ) {
    return;
  }
  throw new Error(
    "vp run build mac now produces notarized artifacts. Set VOYAVPN_NOTARY_KEYCHAIN_PROFILE, or VOYAVPN_NOTARY_APPLE_ID/TEAM_ID/PASSWORD.",
  );
}

function installedExecutableName() {
  const infoPlist = resolve(installedAppBundle, "Contents", "Info.plist");
  if (!existsSync(infoPlist)) {
    return "VoyaVPN";
  }
  return plistBuddy(infoPlist, ":CFBundleExecutable", true) || "VoyaVPN";
}

function installedAppExecutableNames() {
  return new Set([installedExecutableName(), "voyavpn", "VoyaVPN"]);
}

// A `pgrep` that fails outright is an error, not "nothing is running": this
// guards replacing the installed app.
function runningExecutables(executables) {
  return [...new Set(executables)].filter(defaultIsProcessRunning);
}

function assertInstalledAppGuiNotRunning() {
  const running = runningExecutables(installedAppExecutableNames());
  if (running.length) {
    throw new Error(
      `VoyaVPN is still running (${running.join(", ")}). Quit the app before vp run build mac local replaces ${installedAppBundle}.`,
    );
  }
}

function installToApplications() {
  console.log(`Installing ${appBundle} into ${installedAppBundle}`);
  try {
    rmSync(installedAppBundle, { recursive: true, force: true });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      `Unable to replace ${installedAppBundle}: ${message}\nRemove it manually (sudo rm -rf "${installedAppBundle}") and re-run vp run build mac local.`,
      { cause: error },
    );
  }
  run("ditto", [appBundle, installedAppBundle], commandOptions());
  commandStatus("xattr", ["-dr", "com.apple.quarantine", installedAppBundle]);
}

function stripLeftoverPacketTunnelCopies() {
  const leftovers = [
    resolve(appContents, "PlugIns"),
    resolve(repoRoot, "target", "native", "macos", "dmg-staging", "VoyaVPN.app", "Contents", "PlugIns"),
  ];
  for (const path of leftovers) {
    if (!existsSync(path)) {
      continue;
    }
    rmSync(path, { recursive: true, force: true });
    console.log(`Removed leftover PacketTunnel PlugIns copy so it cannot win PlugInKit election: ${path}`);
  }
}

function runNetworkExtensionDoctor(appPath, env, extraArgs = []) {
  const args = ["scripts/native/macos/ne-doctor.mjs", "--fix", "--app", appPath, ...extraArgs];
  if (commandStatus("node", args, env) === 0) {
    return;
  }
  console.warn("PlugInKit election can lag right after registration; retrying the NetworkExtension doctor once...");
  sleepSync(3000);
  run("node", args, commandOptions(env));
}

/**
 * The source-level bridge and provider tests, once per build. They test the
 * sources, not a bundle, so `native macos tunnel verify` — which a lane runs
 * for the app and again for the mounted DMG — does not repeat them.
 */
function runBridgeTests(env) {
  run("node", ["scripts/native/macos/test-bridge.mjs"], commandOptions(env));
}

function requireAppleSilicon() {
  // The store package is arm64 only: the core seed, the Rust binaries and the
  // PacketTunnel are all built for the host architecture.
  if (process.arch !== "arm64") {
    throw new Error(
      "vp run build mac appstore builds the arm64-only store package and must run on an Apple Silicon Mac.",
    );
  }
}

/**
 * The store lane's two distribution profiles, found before anything builds and
 * handed to the child scripts as the explicit profile variables.
 *
 * Without VOYAVPN_PROVISIONING_PROFILE_DIR the search tries ../docs/certs, then
 * the folder macOS installs a double-clicked profile into, which is where store
 * profiles usually are. The criteria are the ones build-tunnel and sign-app
 * apply, so a profile chosen here is one they accept.
 */
function appStoreProvisioningProfiles(identitySha1) {
  const explicitDir = process.env.VOYAVPN_PROVISIONING_PROFILE_DIR?.trim();
  const profileDir = explicitDir
    ? resolve(explicitDir)
    : [defaultProvisioningProfileDir, installedProvisioningProfileDir];
  const criteria = { distribution: "app-store", identitySha1, deviceUdid: localProvisioningUdid() };
  const env = {};
  for (const [bundleIdentifier, explicitEnvName, label] of [
    [appBundleIdentifier, "VOYAVPN_MACOS_APP_PROVISIONING_PROFILE", "macOS app"],
    [packetTunnelBundleIdentifier, "VOYAVPN_PACKET_TUNNEL_PROVISIONING_PROFILE", "PacketTunnel"],
  ]) {
    const { profile, rejections } = resolveProfileFromEnv({ bundleIdentifier, explicitEnvName, profileDir, criteria });
    if (!profile) {
      const fullLabel = `${label} ${distributionProfileLabel("app-store")}`;
      throw new Error(
        `${formatProfileSelectionError(fullLabel, bundleIdentifier, rejections, profileDir)}\nSet ${explicitEnvName} to select a profile explicitly.`,
      );
    }
    console.log(`${label} provisioning profile: ${profile.path}`);
    env[explicitEnvName] = profile.path;
  }
  return env;
}

function buildAppStorePackage() {
  requireAppleSilicon();
  // The store package bundles a sing-box seed compiled from the pinned source
  // (docs/release/macos-app-store.md); fail now rather than after the Rust build.
  const goVersion = requireGoToolchain();
  const identity = appStoreIdentity();
  const commonEnv = {
    ...process.env,
    ...appStoreProvisioningProfiles(identity),
    VOYAVPN_MAC_APP_STORE: "1",
    VOYAVPN_MACOS_APP_BUNDLE: appBundle,
    VOYAVPN_CODESIGN_IDENTITY: identity,
    VOYAVPN_MACOS_DISTRIBUTION: "app-store",
    VOYAVPN_REQUIRE_PROVISIONING: "1",
    VOYAVPN_SING_BOX_SEED_ORIGIN: "source",
  };
  // A development profile would sign and verify, then fail App Review.
  delete commonEnv.VOYAVPN_ALLOW_DEVELOPMENT_PROVISIONING;
  const verifyEnv = {
    ...commonEnv,
    VOYAVPN_REQUIRE_LIBBOX: "1",
    VOYAVPN_REQUIRE_CODESIGN: "1",
  };

  console.log("Building the Mac App Store package (arm64, PacketTunnel appex, no self-updater).");
  console.log(`sing-box seed: built from the pinned source with ${goVersion}.`);
  console.log(`Output: ${appBundle}`);

  // Start from an empty bundle directory: Tauri writes into an existing
  // bundle, so a PlugIns or SystemExtensions copy from another lane would
  // otherwise survive into the store package.
  rmSync(appBundle, { recursive: true, force: true });
  run("vp", ["run", "tauri", "build", "--bundles", "app"], commandOptions(commonEnv));
  run("vp", ["run", "native", "macos", "tunnel"], commandOptions(verifyEnv));
  run("vp", ["run", "native", "macos", "app", "sign"], commandOptions(commonEnv));
  runBridgeTests(verifyEnv);
  run("vp", ["run", "native", "macos", "tunnel", "verify"], commandOptions(verifyEnv));
  run("vp", ["run", "native", "macos", "pkg"], commandOptions(verifyEnv));

  // The signed .pkg carries its own copy of the app. The target/ bundle cannot
  // launch outside the store anyway, and its appex could win PlugInKit
  // election for the production bundle id (AGENTS.md, NetworkExtension hygiene).
  stripLeftoverPacketTunnelCopies();
  console.log("");
  console.log("The target/ app had Contents/PlugIns removed for PlugInKit hygiene; the .pkg above is the artifact.");
}

function main() {
  requireDarwin("vp run build mac must run on macOS.");
  const lane = buildLane();
  if (lane === "app-store") {
    buildAppStorePackage();
    return;
  }
  requireNotaryCredentials(lane);
  const notarizationSkipped = lane === "local";

  if (notarizationSkipped) {
    assertInstalledAppGuiNotRunning();
    run("node", ["scripts/native/macos/preflight.mjs"], commandOptions());
  }

  const identity = notarizationSkipped ? localAppExtensionIdentity() : developerIdIdentity();
  const macosDistribution = notarizationSkipped ? "app-store" : "developer-id";
  const commonEnv = {
    ...process.env,
    VOYAVPN_MACOS_APP_BUNDLE: appBundle,
    VOYAVPN_CODESIGN_IDENTITY: identity,
    VOYAVPN_MACOS_DISTRIBUTION: macosDistribution,
    VOYAVPN_REQUIRE_PROVISIONING: "1",
    ...(notarizationSkipped ? { VOYAVPN_ALLOW_DEVELOPMENT_PROVISIONING: "1" } : {}),
  };
  const tunnelEnv = {
    ...commonEnv,
    VOYAVPN_REQUIRE_LIBBOX: "1",
  };
  const verifyEnv = {
    ...tunnelEnv,
    VOYAVPN_REQUIRE_CODESIGN: "1",
    ...(notarizationSkipped ? {} : { VOYAVPN_REQUIRE_NOTARIZATION_READY: "1" }),
  };

  console.log(
    notarizationSkipped
      ? "Building local macOS app with PacketTunnel appex. Apple notarization is skipped."
      : "Building notarized macOS app with PacketTunnel System Extension.",
  );
  console.log(`Output: ${appBundle}`);

  run("vp", ["run", "tauri", "build", "--bundles", "app"], commandOptions(commonEnv));
  run("vp", ["run", "native", "macos", "tunnel"], commandOptions(tunnelEnv));
  run("vp", ["run", "native", "macos", "app", "sign"], commandOptions(commonEnv));
  runBridgeTests(verifyEnv);
  run("vp", ["run", "native", "macos", "tunnel", "verify"], commandOptions(verifyEnv));

  if (!notarizationSkipped) {
    const appNotarizeEnv = withoutEnv(verifyEnv, ["VOYAVPN_NOTARY_ARTIFACT"]);
    run("vp", ["run", "native", "macos", "app", "notarize"], commandOptions(appNotarizeEnv));
    run("spctl", ["--assess", "--type", "execute", "--verbose=4", appBundle], commandOptions(verifyEnv));
  }

  const finalDmgPath = resolveDmgPath({ appContents, dmgDir, version: packageJson.version });
  const dmgEnv = {
    ...verifyEnv,
    VOYAVPN_MACOS_DMG_PATH: finalDmgPath,
  };
  run("vp", ["run", "native", "macos", "dmg"], commandOptions(dmgEnv));
  if (!notarizationSkipped) {
    run(
      "vp",
      ["run", "native", "macos", "app", "notarize"],
      commandOptions({
        ...dmgEnv,
        VOYAVPN_NOTARY_ARTIFACT: finalDmgPath,
      }),
    );
    run(
      "spctl",
      ["--assess", "--type", "open", "--context", "context:primary-signature", "--verbose=4", finalDmgPath],
      commandOptions(dmgEnv),
    );
  }

  commandStatus("xattr", ["-dr", "com.apple.quarantine", appBundle], commonEnv);
  if (notarizationSkipped) {
    prepareVoyaForLocalBuild({
      guiExecutables: [...installedAppExecutableNames()],
      replacementTarget: installedAppBundle,
    });
    installToApplications();
    stripLeftoverPacketTunnelCopies();
    runNetworkExtensionDoctor(installedAppBundle, commonEnv);
  } else {
    runNetworkExtensionDoctor(appBundle, commonEnv, ["--dev"]);
  }

  const launchBundle = notarizationSkipped ? installedAppBundle : appBundle;
  console.log("");
  console.log(notarizationSkipped ? "Local macOS TUN test app is ready:" : "Notarized macOS app is ready:");
  console.log(`  ${launchBundle}`);
  console.log(`  ${finalDmgPath}`);
  if (notarizationSkipped) {
    console.log("  PacketTunnel appex is staged and signed for local testing only.");
    console.log(
      "  The target/ build copies had Contents/PlugIns removed (PlugInKit hygiene); launch the installed app, and reinstall from the DMG if you need a pristine bundle.",
    );
  } else {
    console.log("  PacketTunnel system extension is staged, signed, notarized, and ready for first-run approval.");
  }
  console.log("");
  console.log("Open it with:");
  console.log(`  open -n ${JSON.stringify(launchBundle)}`);
  console.log("");
  console.log("Do not use vp run tauri dev for macOS TUN testing; it does not bundle the PacketTunnel provider.");
}

// Guarded like sign-app.mjs: importing this module must not start a build.
if (isCliEntrypoint(import.meta.url)) {
  runCli(main);
}
