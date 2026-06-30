import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const appBundle = resolve(process.env.VOYAVPN_MACOS_APP_BUNDLE || resolve(repoRoot, "target", "native", "macos", "VoyaVPN.app"));
const artifact = process.env.VOYAVPN_NOTARY_ARTIFACT ? resolve(process.env.VOYAVPN_NOTARY_ARTIFACT) : null;
const notaryZip = resolve(repoRoot, "target", "native", "macos", "VoyaVPN-notary.zip");

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: options.cwd ?? repoRoot,
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")} failed with status ${result.status}`);
  }
}

function notaryCredentials() {
  if (process.env.VOYAVPN_NOTARY_KEYCHAIN_PROFILE) {
    return ["--keychain-profile", process.env.VOYAVPN_NOTARY_KEYCHAIN_PROFILE];
  }

  const appleId = process.env.VOYAVPN_NOTARY_APPLE_ID;
  const teamId = process.env.VOYAVPN_NOTARY_TEAM_ID;
  const password = process.env.VOYAVPN_NOTARY_PASSWORD;
  if (appleId && teamId && password) {
    return ["--apple-id", appleId, "--team-id", teamId, "--password", password];
  }

  throw new Error(
    "notary credentials are required: set VOYAVPN_NOTARY_KEYCHAIN_PROFILE, or VOYAVPN_NOTARY_APPLE_ID/TEAM_ID/PASSWORD.",
  );
}

function prepareArtifact() {
  if (artifact) {
    if (!existsSync(artifact)) {
      throw new Error(`notary artifact is missing: ${artifact}`);
    }
    return artifact;
  }

  if (!existsSync(appBundle)) {
    throw new Error(`macOS app bundle is missing: ${appBundle}`);
  }

  mkdirSync(dirname(notaryZip), { recursive: true });
  rmSync(notaryZip, { force: true });
  run("ditto", ["-c", "-k", "--keepParent", appBundle, notaryZip], { cwd: dirname(appBundle) });
  return notaryZip;
}

function stapleTarget(submittedArtifact) {
  const extension = extname(submittedArtifact).toLowerCase();
  if (extension === ".dmg" || extension === ".pkg") {
    return submittedArtifact;
  }
  return appBundle;
}

function main() {
  if (process.platform !== "darwin") {
    throw new Error("macOS notarization must run on macOS.");
  }

  const submittedArtifact = prepareArtifact();
  run("xcrun", ["notarytool", "submit", submittedArtifact, "--wait", ...notaryCredentials()]);

  const target = stapleTarget(submittedArtifact);
  run("xcrun", ["stapler", "staple", target]);
  run("xcrun", ["stapler", "validate", target]);
  console.log(`macOS notarization completed for ${target}`);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
