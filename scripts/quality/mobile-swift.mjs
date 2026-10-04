import { readdirSync } from "node:fs";
import { resolve } from "node:path";

import {
  captureSpawned,
  checkedCapture,
  isCliEntrypoint,
  repoRootFromScript,
  runCli,
} from "../lib/common.mjs";
import { packetTunnelSources } from "../native/macos/tunnel-layout.mjs";

/**
 * Parses the iOS app's own Swift — the app delegate, the native modules, the
 * XCUITest bundle beside them — and typechecks the part that can be.
 *
 * These sources have no host-side build: they reach React, NetworkExtension,
 * Libbox and the uniffi bindings, none of which typecheck without an Xcode
 * project and the staged frameworks. What *can* be checked anywhere with the
 * command line tools is that they parse — which catches the mistakes that are
 * otherwise found by a twenty-minute Xcode build, or by nobody until someone
 * tries to ship. The UI tests are included because they otherwise compile
 * only during the simulator lane's `xcodebuild build-for-testing`: that runs
 * for a pull request touching the mobile app, minutes into a macOS job, where
 * this says so in seconds on any machine.
 *
 * The provider sources shared with macOS are typechecked for macOS by
 * `pnpm check:native:macos:bridge`. Their `#if os(iOS)` branches are not seen
 * by that, so they are typechecked for iOS here; outside Libbox's `canImport`
 * they need nothing but the SDK.
 */
const UI_TESTS_DIR = ["apps", "mobile", "ios", "VoyaVPNUITests"];
const SOURCE_DIRS = [["apps", "mobile", "ios", "VoyaVPN", "Native"], UI_TESTS_DIR];
/** `IPHONEOS_DEPLOYMENT_TARGET` of the Xcode project. */
const IOS_DEPLOYMENT_TARGET = "15.1";

/** Single files outside those directories. */
const SOURCE_FILES = [["apps", "mobile", "ios", "VoyaVPN", "AppDelegate.swift"]];

function swiftFilesIn(directory) {
  return readdirSync(directory)
    .filter((name) => name.endsWith(".swift"))
    .sort()
    .map((name) => resolve(directory, name));
}

function swiftSources(root) {
  return [
    ...SOURCE_DIRS.flatMap((segments) => swiftFilesIn(resolve(root, ...segments))),
    ...SOURCE_FILES.map((segments) => resolve(root, ...segments)),
  ];
}

export function main() {
  const root = repoRootFromScript(import.meta.url);
  if (process.platform !== "darwin") {
    // Skipped with evidence rather than silently: a green run on Linux must
    // not read as "the Swift is fine".
    console.log("SKIP iOS Swift parse: swiftc is only available on macOS.");
    return;
  }

  const sources = swiftSources(root);
  const result = captureSpawned("xcrun", ["swiftc", "-parse", ...sources], { cwd: root });
  if (result.status !== 0) {
    throw new Error(`iOS Swift sources did not parse:\n${result.stderr ?? ""}`);
  }
  console.log(`iOS Swift sources parsed: ${sources.length} files.`);

  typecheckUiTests(root);
  typecheckPacketTunnel(root);
}

function typecheckPacketTunnel(root) {
  const sdk = checkedCapture("xcrun", ["--sdk", "iphonesimulator", "--show-sdk-path"]).stdout.trim();
  const sources = packetTunnelSources(root);
  const result = captureSpawned(
    "xcrun",
    [
      "swiftc",
      "-typecheck",
      "-parse-as-library",
      "-sdk",
      sdk,
      "-target",
      `arm64-apple-ios${IOS_DEPLOYMENT_TARGET}-simulator`,
      ...sources,
    ],
    { cwd: root },
  );
  if (result.status !== 0) {
    throw new Error(`PacketTunnel sources did not typecheck for iOS:\n${result.stderr ?? ""}`);
  }
  console.log(`PacketTunnel sources typechecked for iOS without Libbox: ${sources.length} files.`);
}

/**
 * The XCUITest bundle imports only what the simulator SDK ships (XCTest,
 * Vision, UIKit), so unlike the app's own sources it typechecks with nothing
 * generated. A renamed helper or a wrong type is caught here instead of
 * minutes into the simulator lane.
 */
function typecheckUiTests(root) {
  const sdk = checkedCapture("xcrun", ["--sdk", "iphonesimulator", "--show-sdk-path"]).stdout.trim();
  const platform = checkedCapture("xcrun", ["--sdk", "iphonesimulator", "--show-sdk-platform-path"]).stdout.trim();
  const sources = swiftFilesIn(resolve(root, ...UI_TESTS_DIR));
  const result = captureSpawned(
    "xcrun",
    [
      "swiftc",
      "-typecheck",
      "-parse-as-library",
      "-sdk",
      sdk,
      "-target",
      `arm64-apple-ios${IOS_DEPLOYMENT_TARGET}-simulator`,
      "-F",
      resolve(platform, "Developer/Library/Frameworks"),
      "-I",
      resolve(platform, "Developer/usr/lib"),
      ...sources,
    ],
    { cwd: root },
  );
  if (result.status !== 0) {
    throw new Error(`iOS UI tests did not typecheck:\n${result.stderr ?? ""}`);
  }
  console.log(`iOS UI tests typechecked: ${sources.length} files.`);
}

if (isCliEntrypoint(import.meta.url)) {
  runCli(main);
}
