import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const appBundle = resolve(process.env.VOYAVPN_MACOS_APP_BUNDLE || resolve(repoRoot, "target", "native", "macos", "VoyaVPN.app"));
const appEntitlements = resolve(repoRoot, "src-tauri", "entitlements", "macos-app.plist");

function truthy(value) {
  return /^(1|true|yes|on)$/i.test(String(value ?? "").trim());
}

function run(program, args) {
  const result = spawnSync(program, args, {
    cwd: repoRoot,
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")} failed with status ${result.status}`);
  }
}

function main() {
  if (process.platform !== "darwin") {
    throw new Error("macOS app signing must run on macOS.");
  }
  if (!existsSync(appBundle)) {
    throw new Error(`macOS app bundle is missing: ${appBundle}`);
  }

  const identity = process.env.VOYAVPN_CODESIGN_IDENTITY;
  if (!identity) {
    throw new Error("VOYAVPN_CODESIGN_IDENTITY is required to sign the macOS app bundle.");
  }

  const args = ["--force", "--deep", "--options", "runtime", "--sign", identity, "--entitlements", appEntitlements];
  if (!truthy(process.env.VOYAVPN_DISABLE_CODESIGN_TIMESTAMP)) {
    args.push("--timestamp");
  }
  args.push(appBundle);

  run("codesign", args);
  run("codesign", ["--verify", "--deep", "--strict", "--verbose=2", appBundle]);
  run("spctl", ["--assess", "--type", "execute", "--verbose=4", appBundle]);
  console.log(`macOS app bundle signed: ${appBundle}`);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
