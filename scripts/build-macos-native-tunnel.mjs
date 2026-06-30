import { cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const nativeRoot = resolve(repoRoot, "src-tauri", "native", "macos");
const outRoot = resolve(repoRoot, "target", "native", "macos");
const appBundle = resolve(process.env.VOYAVPN_MACOS_APP_BUNDLE || resolve(outRoot, "VoyaVPN.app"));
const appContents = resolve(appBundle, "Contents");
const helperSource = resolve(nativeRoot, "TunnelHelper", "VoyaPacketTunnelManager.swift");
const providerSource = resolve(nativeRoot, "PacketTunnel", "PacketTunnelProvider.swift");
const helperOut = resolve(appContents, "MacOS", "voyavpn-macos-tunnelctl");
const appexContents = resolve(appContents, "PlugIns", "app.voyavpn.desktop.PacketTunnel.appex", "Contents");
const appexBundle = resolve(appexContents, "..");
const appexBinary = resolve(appexContents, "MacOS", "VoyaPacketTunnel");
const appexFrameworks = resolve(appexContents, "Frameworks");
const defaultLibboxXCFramework = resolve(nativeRoot, "Frameworks", "Libbox.xcframework");
const libboxXCFramework = resolve(process.env.VOYAVPN_LIBBOX_XCFRAMEWORK || defaultLibboxXCFramework);
const embeddedLibboxFramework = resolve(appexFrameworks, "Libbox.framework");
const appEntitlements = resolve(repoRoot, "src-tauri", "entitlements", "macos-app.plist");
const packetTunnelEntitlements = resolve(repoRoot, "src-tauri", "entitlements", "packet-tunnel.plist");

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

function requireDarwin() {
  if (process.platform !== "darwin") {
    throw new Error("macOS native tunnel build must run on macOS with Xcode command line tools.");
  }
}

function writePlist(source, destination, replacements = {}) {
  let text = readFileSync(source, "utf8");
  for (const [from, to] of Object.entries(replacements)) {
    text = text.replaceAll(from, to);
  }
  writeFileSync(destination, text);
}

function collectDirectories(root, predicate, results = []) {
  if (!existsSync(root)) {
    return results;
  }

  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (!entry.isDirectory()) {
      continue;
    }
    if (predicate(path, entry.name)) {
      results.push(path);
      continue;
    }
    collectDirectories(path, predicate, results);
  }

  return results;
}

function libboxPreferenceScore(path) {
  const normalized = path.toLowerCase();
  let score = 0;
  if (normalized.includes("macos")) {
    score += 100;
  }
  if (normalized.includes("arm64_x86_64") || normalized.includes("x86_64_arm64")) {
    score += 20;
  }
  if (normalized.includes(process.arch === "arm64" ? "arm64" : "x86_64")) {
    score += 10;
  }
  if (normalized.includes("ios")) {
    score -= 100;
  }
  return score;
}

function findLibboxFramework() {
  if (!existsSync(libboxXCFramework)) {
    return null;
  }
  if (!statSync(libboxXCFramework).isDirectory()) {
    throw new Error(`VOYAVPN_LIBBOX_XCFRAMEWORK is not a directory: ${libboxXCFramework}`);
  }

  const frameworks = collectDirectories(libboxXCFramework, (_path, name) => name === "Libbox.framework");
  if (!frameworks.length) {
    throw new Error(`Libbox.framework was not found inside ${libboxXCFramework}`);
  }

  frameworks.sort((left, right) => libboxPreferenceScore(right) - libboxPreferenceScore(left));
  return frameworks[0];
}

function buildHelper() {
  mkdirSync(dirname(helperOut), { recursive: true });
  run("xcrun", [
    "swiftc",
    "-O",
    "-parse-as-library",
    "-framework",
    "Foundation",
    "-framework",
    "NetworkExtension",
    helperSource,
    "-o",
    helperOut,
  ]);
}

function buildPacketTunnel() {
  const libboxFramework = findLibboxFramework();
  if (!libboxFramework) {
    const message = `Libbox.xcframework not found at ${libboxXCFramework}; PacketTunnel will build but fail closed until the framework is provided.`;
    if (/^(1|true|yes|on)$/i.test(String(process.env.VOYAVPN_REQUIRE_LIBBOX ?? ""))) {
      throw new Error(message);
    }
    console.warn(message);
  }

  mkdirSync(dirname(appexBinary), { recursive: true });
  const args = [
    "swiftc",
    "-O",
    "-emit-library",
    "-emit-module",
    "-module-name",
    "VoyaPacketTunnel",
    "-framework",
    "Foundation",
    "-framework",
    "NetworkExtension",
    "-framework",
    "Network",
  ];

  if (libboxFramework) {
    args.push(
      "-F",
      dirname(libboxFramework),
      "-framework",
      "Libbox",
      "-Xlinker",
      "-rpath",
      "-Xlinker",
      "@executable_path/../Frameworks",
    );
  }

  args.push(providerSource, "-o", appexBinary);
  run("xcrun", args);

  writePlist(
    resolve(nativeRoot, "PacketTunnel", "Info.plist"),
    resolve(appexContents, "Info.plist"),
    {
      "$(PRODUCT_MODULE_NAME)": "VoyaPacketTunnel",
      "$(EXECUTABLE_NAME)": "VoyaPacketTunnel",
      "$(MARKETING_VERSION)": "0.1.0",
      "$(CURRENT_PROJECT_VERSION)": "1",
    },
  );

  if (libboxFramework) {
    rmSync(embeddedLibboxFramework, { force: true, recursive: true });
    mkdirSync(appexFrameworks, { recursive: true });
    cpSync(libboxFramework, embeddedLibboxFramework, {
      dereference: false,
      force: true,
      recursive: true,
      verbatimSymlinks: true,
    });
    console.log(`Embedded Libbox.framework from ${libboxFramework}`);
  }
}

function maybeCodesign() {
  const identity = process.env.VOYAVPN_CODESIGN_IDENTITY;
  if (!identity) {
    console.warn("Skipping codesign: VOYAVPN_CODESIGN_IDENTITY is not set.");
    console.warn(`App entitlements: ${appEntitlements}`);
    console.warn(`PacketTunnel entitlements: ${packetTunnelEntitlements}`);
    return;
  }

  if (existsSync(embeddedLibboxFramework)) {
    run("codesign", ["--force", "--sign", identity, embeddedLibboxFramework]);
  }
  run("codesign", ["--force", "--sign", identity, "--entitlements", packetTunnelEntitlements, appexBundle]);
  run("codesign", ["--force", "--sign", identity, "--entitlements", appEntitlements, helperOut]);
}

function main() {
  requireDarwin();
  if (!existsSync(helperSource) || !existsSync(providerSource)) {
    throw new Error("macOS native tunnel sources are missing.");
  }
  buildHelper();
  buildPacketTunnel();
  maybeCodesign();
  console.log(`macOS native tunnel staged in ${appBundle}`);
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
