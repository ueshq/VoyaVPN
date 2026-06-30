import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const appBundle = resolve(
  process.env.VOYAVPN_MACOS_APP_BUNDLE || resolve(repoRoot, "target", "native", "macos", "VoyaVPN.app"),
);
const appContents = resolve(appBundle, "Contents");
const helper = resolve(appContents, "MacOS", "voyavpn-macos-tunnelctl");
const appex = resolve(appContents, "PlugIns", "app.voyavpn.desktop.PacketTunnel.appex");
const appexContents = resolve(appex, "Contents");
const appexBinary = resolve(appexContents, "MacOS", "VoyaPacketTunnel");
const libbox = resolve(appexContents, "Frameworks", "Libbox.framework");

function truthy(value) {
  return /^(1|true|yes|on)$/i.test(String(value ?? "").trim());
}

function run(program, args) {
  return spawnSync(program, args, {
    cwd: repoRoot,
    encoding: "utf8",
  });
}

function requireDarwin() {
  if (process.platform !== "darwin") {
    throw new Error("macOS native tunnel verification must run on macOS.");
  }
}

function requirePath(path, label) {
  if (!existsSync(path)) {
    throw new Error(`${label} is missing: ${path}`);
  }
  console.log(`✓ ${label}: ${path}`);
}

function verifySignature(path, label, requiredEntitlements = []) {
  const verify = run("codesign", ["--verify", "--strict", "--verbose=2", path]);
  if (verify.status !== 0) {
    const message = `${label} is not signed or failed signature verification.`;
    if (truthy(process.env.VOYAVPN_REQUIRE_CODESIGN)) {
      throw new Error(`${message}\n${verify.stderr || verify.stdout}`);
    }
    console.warn(`! ${message}`);
    return;
  }

  console.log(`✓ ${label} signature is valid`);

  if (!requiredEntitlements.length) {
    return;
  }

  const entitlements = run("codesign", ["-d", "--entitlements", ":-", path]);
  const output = `${entitlements.stdout ?? ""}\n${entitlements.stderr ?? ""}`;
  for (const entitlement of requiredEntitlements) {
    if (!output.includes(entitlement)) {
      const message = `${label} signature does not include ${entitlement}.`;
      if (truthy(process.env.VOYAVPN_REQUIRE_CODESIGN)) {
        throw new Error(message);
      }
      console.warn(`! ${message}`);
    } else {
      console.log(`✓ ${label} entitlement includes ${entitlement}`);
    }
  }
}

function main() {
  requireDarwin();
  requirePath(helper, "Tunnel helper");
  requirePath(appex, "PacketTunnel appex");
  requirePath(appexBinary, "PacketTunnel binary");

  if (existsSync(libbox)) {
    console.log(`✓ Embedded Libbox.framework: ${libbox}`);
    verifySignature(libbox, "Libbox.framework");
  } else {
    const message = `Embedded Libbox.framework is missing: ${libbox}`;
    if (truthy(process.env.VOYAVPN_REQUIRE_LIBBOX)) {
      throw new Error(message);
    }
    console.warn(`! ${message}`);
  }

  verifySignature(helper, "Tunnel helper", [
    "com.apple.security.application-groups",
    "group.app.voyavpn.desktop",
  ]);
  verifySignature(appex, "PacketTunnel appex", [
    "com.apple.developer.networking.networkextension",
    "packet-tunnel-provider",
    "com.apple.security.application-groups",
    "group.app.voyavpn.desktop",
  ]);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
