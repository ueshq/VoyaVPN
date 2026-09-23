import { runOrExit } from "../lib/common.mjs";

runOrExit("cargo", ["test", "--workspace", "--all-targets", "--exclude", "voyavpn"]);
runOrExit("cargo", ["test", "-p", "voyavpn", "--bin", "voyavpn"]);
runOrExit("cargo", ["test", "-p", "voyavpn", "--bin", "voyavpn-tunnel-service"]);

if (process.platform === "darwin") {
  runOrExit(process.execPath, ["scripts/native/macos/test-bridge.mjs"]);
}
