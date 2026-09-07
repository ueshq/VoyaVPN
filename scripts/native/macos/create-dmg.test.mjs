import { describe, expect, it } from "vitest";

import { dmgSigningPlan } from "./create-dmg.mjs";

describe("macOS DMG signing", () => {
  it("signs Developer ID disk images with a secure timestamp", () => {
    expect(dmgSigningPlan({ distribution: "developer-id", identity: "1A2B3C" })).toEqual({
      sign: true,
      args: ["--force", "--sign", "1A2B3C", "--timestamp"],
    });
  });

  it("omits the timestamp when timestamping is disabled for offline signing", () => {
    expect(
      dmgSigningPlan({ distribution: "developer-id", identity: "1A2B3C", disableTimestamp: true }).args,
    ).toEqual(["--force", "--sign", "1A2B3C"]);
  });

  it("skips signing for App Store distribution, which ships through App Store Connect", () => {
    const plan = dmgSigningPlan({ distribution: "app-store", identity: "1A2B3C" });

    expect(plan.sign).toBe(false);
    expect(plan.reason).toMatch(/does not ship a Developer ID signed disk image/);
  });

  it("fails closed when a Developer ID build has no signing identity", () => {
    expect(() => dmgSigningPlan({ distribution: "developer-id", identity: "  ", requireCodesign: true })).toThrow(
      /VOYAVPN_CODESIGN_IDENTITY is required/,
    );
    expect(dmgSigningPlan({ distribution: "developer-id", identity: undefined })).toMatchObject({ sign: false });
  });
});
