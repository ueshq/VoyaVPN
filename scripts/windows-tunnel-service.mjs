import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const command = process.argv[2] ?? "help";
const serviceName = "VoyaVPNTunnelService";
const serviceDisplayName = "VoyaVPN Tunnel Service";
const serviceBin = resolve(repoRoot, "target", "release", process.platform === "win32" ? "voyavpn-tunnel-service.exe" : "voyavpn-tunnel-service");

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: repoRoot,
    shell: false,
    stdio: "inherit",
    ...options,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${program} ${args.join(" ")} failed with status ${result.status}`);
  }
}

function requireWindows() {
  if (process.platform !== "win32") {
    throw new Error("Windows tunnel service install commands must run on Windows.");
  }
}

function build() {
  run("cargo", ["build", "-p", "voyavpn", "--bin", "voyavpn-tunnel-service", "--release"]);
}

function install() {
  requireWindows();
  if (!existsSync(serviceBin)) {
    build();
  }
  run("sc.exe", [
    "create",
    serviceName,
    `binPath=`,
    serviceBin,
    "start=",
    "demand",
    "DisplayName=",
    serviceDisplayName,
  ]);
  run("sc.exe", ["description", serviceName, "Runs VoyaVPN transparent TUN with sing-box and Wintun."]);
}

function uninstall() {
  requireWindows();
  spawnSync("sc.exe", ["stop", serviceName], { cwd: repoRoot, stdio: "ignore" });
  run("sc.exe", ["delete", serviceName]);
}

function status() {
  requireWindows();
  run("sc.exe", ["query", serviceName]);
}

function help() {
  console.log("usage: node scripts/windows-tunnel-service.mjs <build|install|uninstall|status>");
}

try {
  switch (command) {
    case "build":
      build();
      break;
    case "install":
      install();
      break;
    case "uninstall":
      uninstall();
      break;
    case "status":
      status();
      break;
    case "help":
    case "--help":
    case "-h":
      help();
      break;
    default:
      throw new Error(`unknown command: ${command}`);
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
