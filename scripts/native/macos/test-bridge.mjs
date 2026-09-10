import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { repoRootFromScript, requireDarwin, run } from "../../lib/common.mjs";

requireDarwin("PacketTunnel native bridge tests require macOS.");
const root = repoRootFromScript(import.meta.url);
const directory = mkdtempSync(join(tmpdir(), "voya-tunnel-wait-"));
try {
  const binary = join(directory, "tunnel-wait-tests");
  run("xcrun", ["clang", "-fobjc-arc", "-fblocks", "-Wall", "-Wextra", "-Werror",
    resolve(root, "crates/voya-platform/native/macos_tunnel_wait_tests.m"),
    "-framework", "Foundation", "-framework", "NetworkExtension", "-o", binary]);
  run(binary, []);
} finally {
  rmSync(directory, { recursive: true, force: true });
}
