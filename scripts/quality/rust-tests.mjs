import { capture } from "../lib/common.mjs";

function runOrExit(command, args, options = {}) {
  console.log(`$ ${[command, ...args].join(" ")}`);
  const result = capture(command, args, {
    ...options,
    stdio: "inherit",
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

runOrExit("cargo", ["test", "--workspace", "--all-targets", "--exclude", "voyavpn"]);
runOrExit("cargo", ["test", "-p", "voyavpn", "--bin", "voyavpn"]);
runOrExit("cargo", ["test", "-p", "voyavpn", "--bin", "voyavpn-tunnel-service"]);

if (process.platform === "darwin") {
  runOrExit(process.execPath, ["scripts/native/macos/test-bridge.mjs"]);
}
