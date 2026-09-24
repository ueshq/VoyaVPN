import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { podsUpToDate, recordInstalledPods } from "./ios-pods-cache.mjs";
import { test } from "vitest";
import { lanAddress, startFixtures } from "./ios-fixtures.mjs";
import { selectRuntime } from "./ios-smoke.mjs";

test("subscription fixture never falls back to forbidden loopback", () => {
  assert.throws(() => lanAddress({ lo: [{ family: "IPv4", internal: true, address: "127.0.0.1" }] }), /private LAN/);
  assert.equal(lanAddress({ eth: [{ family: "IPv4", internal: false, address: "192.168.2.9" }] }), "192.168.2.9");
});
test("selects a usable iOS runtime by numeric version", () => {
  const entry = (version, isAvailable = true) => ({ version, isAvailable, identifier: `com.apple.CoreSimulator.SimRuntime.iOS-${version}` });
  assert.equal(selectRuntime([entry("26.9"), entry("26.10"), entry("27.0", false)]), entry("26.10").identifier);
  assert.throws(() => selectRuntime([]), /Install an iOS/);
});
test("subscription updates serve changed real links and the latency target responds", async () => {
  const fixture = await startFixtures({ device: () => "unused", address: "192.168.2.9", record: () => {} });
  try {
    for (const title of ["A", "Updated", "Refreshed"]) {
      const response = await fetch(fixture.controlUrl + "/subscription");
      assert.equal(response.headers.get("profile-title"), "QA Subscription");
      assert.match(await response.text(), new RegExp(`QA%20Subscription%20${title}`));
    }
    assert.equal((await fetch(fixture.controlUrl + "/ping")).status, 204);
  } finally { await fixture.close(); }
});

test("Pods are reused only with matching locks, project and dependency inputs", () => {
  const root = mkdtempSync(join(tmpdir(), "voya-pods-"));
  try {
    const ios = join(root, "apps/mobile/ios");
    mkdirSync(join(ios, "Pods/Pods.xcodeproj"), { recursive: true });
    writeFileSync(join(ios, "Podfile.lock"), "pod v1");
    writeFileSync(join(ios, "Pods/Manifest.lock"), "pod v1");
    writeFileSync(join(ios, "Pods/Pods.xcodeproj/project.pbxproj"), "project");
    assert.equal(podsUpToDate(root), false);
    recordInstalledPods(root);
    assert.equal(podsUpToDate(root), true);
    writeFileSync(join(root, "pnpm-lock.yaml"), "new native dependency");
    assert.equal(podsUpToDate(root), false);
    recordInstalledPods(root);
    writeFileSync(join(ios, "Pods/Manifest.lock"), "out of sync");
    assert.equal(podsUpToDate(root), false);
    assert.throws(() => recordInstalledPods(root), /consistent lock/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
