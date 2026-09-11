import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { repoRootFromScript, requireDarwin, run } from "../../lib/common.mjs";
import { packetTunnelSources } from "./tunnel-layout.mjs";

requireDarwin("PacketTunnel native bridge tests require macOS.");
const root = repoRootFromScript(import.meta.url);
const directory = mkdtempSync(join(tmpdir(), "voya-tunnel-wait-"));
try {
  const binary = join(directory, "tunnel-wait-tests");
  run("xcrun", ["clang", "-fobjc-arc", "-fblocks", "-Wall", "-Wextra", "-Werror",
    resolve(root, "crates/voya-platform/native/macos_tunnel_wait_tests.m"),
    "-framework", "Foundation", "-framework", "NetworkExtension", "-o", binary]);
  run(binary, []);
  const chromeBinary = join(directory, "window-chrome-tests");
  run("xcrun", ["clang", "-fobjc-arc", "-fblocks", "-Wall", "-Wextra", "-Werror",
    resolve(root, "crates/voya-platform/native/macos_window_chrome.m"),
    resolve(root, "crates/voya-platform/native/macos_window_chrome_tests.m"),
    "-framework", "AppKit", "-o", chromeBinary]);
  run(chromeBinary, []);

  const nativeRoot = resolve(root, "apps/desktop/src-tauri/native/macos");
  const sources = packetTunnelSources(nativeRoot);
  run("xcrun", ["swiftc", "-typecheck", "-parse-as-library", ...sources]);
  console.log("PacketTunnel sources typechecked without Libbox.");
  const framework = resolve(process.env.VOYAVPN_LIBBOX_FRAMEWORK || join(nativeRoot, "Frameworks/Libbox.framework"));
  if (existsSync(framework)) {
    run("xcrun", ["swiftc", "-typecheck", "-parse-as-library", "-F", dirname(framework), ...sources]);
    console.log("PacketTunnel sources typechecked with Libbox.");
  } else {
    console.log(`SKIP PacketTunnel Libbox typecheck: framework absent at ${framework}`);
  }
  const runtimeBinary = join(directory, "packet-tunnel-runtime-tests");
  run("xcrun", ["swiftc", "-parse-as-library",
    join(nativeRoot, "PacketTunnel/PacketTunnelRuntime.swift"),
    join(nativeRoot, "PacketTunnel/PacketTunnelDiagnostics.swift"),
    join(nativeRoot, "PacketTunnelTests.swift"), "-o", runtimeBinary]);
  run(runtimeBinary, []);
} finally {
  rmSync(directory, { recursive: true, force: true });
}
