import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import { capture } from "../../lib/common.mjs";
import { findQuarantined, quarantinedPaths } from "./quarantine.mjs";

describe("quarantine scan (ITMS-91109)", () => {
  const root = "/x/pkg-verify";

  // The shape of `xattr -r` on the expanded build 352 that Transporter rejected.
  it("finds the quarantined profiles and ignores other attributes", () => {
    const listing = [
      `${root}/app.voyavpn.desktop.pkg/Payload/VoyaVPN.app: com.apple.provenance`,
      `${root}/app.voyavpn.desktop.pkg/Payload/VoyaVPN.app/Contents/embedded.provisionprofile: com.apple.provenance`,
      `${root}/app.voyavpn.desktop.pkg/Payload/VoyaVPN.app/Contents/embedded.provisionprofile: com.apple.quarantine`,
      `${root}/app.voyavpn.desktop.pkg/Payload/VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex/Contents/embedded.provisionprofile: com.apple.quarantine`,
      `${root}/app.voyavpn.desktop.pkg/Payload/VoyaVPN.app/Contents/MacOS/voyavpn: com.apple.cs.CodeSignature`,
      "",
    ].join("\n");

    expect(quarantinedPaths(listing, root)).toEqual([
      "app.voyavpn.desktop.pkg/Payload/VoyaVPN.app/Contents/embedded.provisionprofile",
      "app.voyavpn.desktop.pkg/Payload/VoyaVPN.app/Contents/PlugIns/VoyaPacketTunnel.appex/Contents/embedded.provisionprofile",
    ]);
  });

  it("keeps paths that contain spaces or a colon", () => {
    expect(quarantinedPaths(`${root}/My Builds/a: b.txt: com.apple.quarantine`, root)).toEqual(["My Builds/a: b.txt"]);
    expect(quarantinedPaths(`${root}: com.apple.quarantine`, root)).toEqual(["."]);
    expect(quarantinedPaths("", root)).toEqual([]);
  });
});

describe("findQuarantined", () => {
  it.runIf(process.platform === "darwin")("lists quarantined files under a real bundle folder", () => {
    const root = mkdtempSync(join(tmpdir(), "voya-qtn-"));
    try {
      const contents = join(root, "Probe.app", "Contents");
      mkdirSync(contents, { recursive: true });
      writeFileSync(join(contents, "embedded.provisionprofile"), "profile");
      writeFileSync(join(contents, "Info.plist"), "plist");
      const quarantine = "0281;6a46dacf;;D5853559-011B-43DD-B861-15287EBF3D57";
      expect(
        capture("xattr", ["-w", "com.apple.quarantine", quarantine, join(contents, "embedded.provisionprofile")]).status,
      ).toBe(0);

      expect(findQuarantined(root)).toEqual(["Probe.app/Contents/embedded.provisionprofile"]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
