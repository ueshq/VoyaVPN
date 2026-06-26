import { spawnSync } from "node:child_process";

function run(command, args, options = {}) {
  console.log(`$ ${[command, ...args].join(" ")}`);
  const result = spawnSync(command, args, {
    ...options,
    stdio: "inherit",
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function rustHostTriple() {
  const result = spawnSync("rustc", ["-vV"], {
    encoding: "utf8",
  });

  if (result.status !== 0) {
    process.stderr.write(result.stderr || "failed to detect rustc host triple\n");
    process.exit(result.status ?? 1);
  }

  const hostLine = result.stdout
    .split(/\r?\n/)
    .find((line) => line.startsWith("host: "));

  return hostLine?.slice("host: ".length).trim() ?? "";
}

const host = rustHostTriple();

if (host.endsWith("-pc-windows-gnu")) {
  console.log(
    "Detected windows-gnu Rust host; skipping the Tauri lib test harness because it fails during WebView2/Wry DLL load on this unsupported local target.",
  );
  run("cargo", ["test", "--workspace", "--all-targets", "--exclude", "voyavpn"]);
  run("cargo", ["test", "-p", "voyavpn", "--bin", "voyavpn"]);
} else {
  run("cargo", ["test", "--workspace", "--all-targets"]);
}
