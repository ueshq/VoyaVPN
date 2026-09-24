import { spawn } from "node:child_process";
import { appendFileSync, cpSync, existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { setTimeout as pause } from "node:timers/promises";
import { isCliEntrypoint, repoRootFromScript } from "../../lib/common.mjs";
import { podsUpToDate, recordInstalledPods } from "./ios-pods-cache.mjs";
import { lanAddress, startFixtures, unusedPort } from "./ios-fixtures.mjs";

const root = repoRootFromScript(import.meta.url);
const ios = resolve(root, "apps/mobile/ios");
const bundleId = "app.voyavpn.mobile";
const smokeTests = ["testLaunchAndAllPages", "testNodeImportShareQrDelete", "testSubscriptionAutoRefreshAndPolicyGroup", "testRealLatencyTimeoutCancelAndRetry", "testRulesAndSettingsPersist", "testDnsValidationAndPersistence"];

export function selectRuntime(runtimes) {
  const runtime = runtimes.filter((item) => item.isAvailable && item.identifier.includes(".iOS-"))
    .sort((a, b) => b.version.localeCompare(a.version, undefined, { numeric: true }))[0];
  if (!runtime) throw new Error("Install an iOS Simulator runtime in Xcode Settings > Platforms.");
  return runtime.identifier;
}

export async function main() {
  const matrixOnly = process.argv.includes("--matrix-only");
  const full = process.argv.includes("--full") || matrixOnly;
  const output = resolve(root, process.env.VOYA_IOS_QA_OUTPUT || `.agents/docs/ios-repair-${new Date().toISOString().replace(/[:.]/g, "-")}`);
  mkdirSync(output, { recursive: true });
  const children = new Set();
  const devices = [];
  const results = [];
  let activeDevice;
  let fixtures;
  let phase = "environment";
  let commandNumber = 0;
  const record = (line) => appendFileSync(resolve(output, "fixtures.log"), line + "\n");
  // Async child processes keep the local HTTP fixture responsive during XCTest.
  async function run(command, args, { cwd = root, input, allowFailure = false, label = command, env = {}, timeoutMs = 30 * 60 * 1000 } = {}) {
    const log = resolve(output, `${String(++commandNumber).padStart(3, "0")}-${label.replace(/[^a-z0-9-]/gi, "-")}.log`);
    return await new Promise((accept, reject) => {
      const child = spawn(command, args, { cwd, timeout: timeoutMs, env: { ...process.env, ...env } });
      children.add(child);
      let stdout = "";
      const append = (data) => appendFileSync(log, data);
      child.stdout.on("data", (data) => { stdout += data; append(data); });
      child.stderr.on("data", append);
      child.once("error", reject);
      child.once("close", (code) => {
        children.delete(child);
        if (code !== 0 && !allowFailure) reject(new Error(`${label} exited ${code}; see ${log}`));
        else accept({ stdout: stdout.trim(), code });
      });
      child.stdin.end(input);
    });
  }
  const sim = (args, options) => run("xcrun", ["simctl", ...args], { label: `simctl-${args[0]}`, ...options });
  const cleanupSignal = () => { for (const child of children) child.kill("SIGTERM"); };
  process.once("SIGINT", cleanupSignal);
  process.once("SIGTERM", cleanupSignal);
  try {
    if (process.platform !== "darwin" || process.arch !== "arm64") throw new Error("Requires an Apple silicon Mac with Xcode, CocoaPods, Rust and pnpm installed.");
    // A visible Simulator can overwrite a device clipboard from the host.
    // Do not change the developer's global setting; reject an unsafe fixture
    // environment before booting or writing any synthetic clipboard content.
    const simulatorGui = await run("pgrep", ["-x", "Simulator"], { allowFailure: true });
    if (simulatorGui.code === 0) {
      const sync = await run("defaults", ["read", "com.apple.iphonesimulator", "PasteboardAutomaticSync"], { allowFailure: true });
      if (sync.stdout !== "0") throw new Error("In Simulator, disable Edit > Automatically Sync Pasteboard before running clipboard regression tests.");
    }
    const address = lanAddress();
    const xcode = await run("xcodebuild", ["-version"]);
    await run("python3", ["-c", "import sqlite3, plistlib"]);
    const runtime = selectRuntime(JSON.parse((await sim(["list", "runtimes", "--json"])).stdout).runtimes);
    const types = JSON.parse((await sim(["list", "devicetypes", "--json"])).stdout).devicetypes;
    const names = full ? ["iPhone 17", "iPhone SE (3rd generation)", "iPhone 17 Pro Max"] : ["iPhone 17"];
    const selected = names.map((name) => {
      const type = types.find((item) => item.name === name);
      if (!type) throw new Error(`Missing simulator device type: ${name}`);
      return type;
    });
    writeFileSync(resolve(output, "environment.json"), JSON.stringify({ xcode: xcode.stdout, runtime, devices: selected, address, full, matrixOnly }, null, 2));
    console.log(`iOS ${full ? "full matrix" : "smoke"}; evidence: ${output}`);
    await run("rustup", ["target", "add", "aarch64-apple-ios", "aarch64-apple-ios-sim"]);
    await run("pnpm", ["native:mobile:rust:ios"]);
    const libbox = resolve(ios, "Frameworks/Libbox.xcframework");
    // The build script owns the pinned version. CI builds it fresh; a developer
    // may reuse their staged copy explicitly while iterating the same pin.
    if (!process.argv.includes("--reuse-libbox") || !existsSync(libbox)) await run("pnpm", ["native:mobile:libbox:ios"]);
    if (!podsUpToDate(root)) {
      await run("pod", ["install"], { cwd: ios });
      recordInstalledPods(root);
    } else console.log("Pods match the dependency and Podfile fingerprints.");
    await run("pnpm", ["native:mobile:ios:project"]);
    await run("node", ["scripts/core/install-sing-box.mjs"]);
    const derived = resolve(ios, "build/DerivedData");
    const buildArgs = ["-workspace", resolve(ios, "VoyaVPN.xcworkspace"), "-scheme", "VoyaVPN", "-configuration", "Release", "-derivedDataPath", derived, "CODE_SIGNING_ALLOWED=NO", "ONLY_ACTIVE_ARCH=YES", "ARCHS=arm64"];
    for (let index = 0; index < selected.length; index += 1) {
      const type = selected[index];
      activeDevice = (await sim(["create", `VoyaVPN Regression ${Date.now()}`, type.identifier, runtime])).stdout;
      devices.push(activeDevice);
      console.log(`Preparing ${type.name}: ${activeDevice}`);
      if (index === 0) {
        phase = "build";
        await run("xcodebuild", ["build-for-testing", ...buildArgs, "-destination", "generic/platform=iOS Simulator"], { label: "build-for-testing" });
      }
      phase = "environment";
      await sim(["boot", activeDevice]);
      await sim(["bootstatus", activeDevice, "-b"], { timeoutMs: 10 * 60 * 1000 });
      const products = resolve(derived, "Build/Products");
      const builtApp = resolve(products, "Release-iphonesimulator/VoyaVPN.app");
      await sim(["install", activeDevice, builtApp]);
      await sim(["launch", activeDevice, bundleId]);
      const container = (await sim(["get_app_container", activeDevice, bundleId, "data"])).stdout;
      fixtures = await startFixtures({ device: () => activeDevice, address, record });
      // Startup must create its own schema/defaults before endpoint-only setup.
      const databaseReady = async () => {
        const result = await run("python3", [resolve(root, "scripts/native/mobile/ios-test-settings.py"), "database", container, fixtures.controlUrl], { allowFailure: true });
        return result.code === 0;
      };
      let ready = false;
      for (let attempt = 0; attempt < 30 && !ready; attempt += 1) { ready = await databaseReady(); if (!ready) await pause(500); }
      if (!ready) { phase = "product"; throw new Error("Native backend did not initialize its database."); }
      await sim(["terminate", activeDevice, bundleId]);
      await databaseReady();
      const vlessPort = await unusedPort();
      const config = resolve(output, `vless-${index}.json`);
      writeFileSync(config, JSON.stringify({ log: { level: "error" }, inbounds: [{ type: "vless", listen: "127.0.0.1", listen_port: vlessPort, users: [{ uuid: "44444444-4444-4444-4444-444444444444" }] }], outbounds: [{ type: "direct" }] }));
      const core = spawn(resolve(root, "apps/desktop/src-tauri/resources/core-seeds/sing_box/sing-box"), ["run", "-c", config]);
      children.add(core);
      core.stdout.on("data", (data) => appendFileSync(resolve(output, "vless.log"), data));
      core.stderr.on("data", (data) => appendFileSync(resolve(output, "vless.log"), data));
      core.once("exit", () => children.delete(core));
      core.once("error", (error) => {
        children.delete(core);
        record(`VLESS fixture launch failed: ${error}`);
        // The readiness check below rejects through main's finally cleanup.
      });
      await run("python3", ["-c", "import socket,sys,time\nfor i in range(100):\n try:\n  s=socket.create_connection(('127.0.0.1',int(sys.argv[1])),.2);s.close();break\n except OSError: time.sleep(.1)\nelse: raise RuntimeError('VLESS fixture did not start')", String(vlessPort)]);
      const template = readdirSync(products).find((file) => file.endsWith(".xctestrun") && !file.startsWith("VoyaRegression-"));
      if (!template) throw new Error("build-for-testing did not produce an xctestrun file");
      const testRun = resolve(products, `VoyaRegression-${index}.xctestrun`);
      cpSync(resolve(products, template), testRun);
      await run("python3", [resolve(root, "scripts/native/mobile/ios-test-settings.py"), "xctestrun", testRun, JSON.stringify({ QA_CONTROL_URL: fixtures.controlUrl, QA_SUBSCRIPTION_URL: fixtures.subscriptionUrl, QA_VLESS_PORT: String(vlessPort) })]);
      phase = "product";
      if (index === 0) {
        // Compile the actual host against the real pinned framework, with only
        // its envelope/container replaced; verify lifecycle on a long sandbox path.
        const slice = readdirSync(libbox).find((name) => name.includes("simulator"));
        const sdk = (await run("xcrun", ["--sdk", "iphonesimulator", "--show-sdk-path"])).stdout;
        const executable = resolve(output, "probe-core-tests");
        phase = "build";
        await run("xcrun", ["swiftc", "-sdk", sdk, "-target", "arm64-apple-ios16.0-simulator", "-F", resolve(libbox, slice), "-framework", "Libbox", "-framework", "UIKit", "-framework", "CoreGraphics", "-framework", "CoreText", "-framework", "CoreTelephony", "-framework", "UniformTypeIdentifiers", "-framework", "SystemConfiguration", "-framework", "Network", "-lresolv", "-Xlinker", "-dead_strip", "-Xlinker", "-no_compact_unwind", resolve(root, "scripts/native/mobile/ProbeCoreTests.swift"), resolve(ios, "VoyaVPN/Native/LibboxProbeCoreHost.swift"), "-o", executable], { label: "probe-build" });
        phase = "product";
        await sim(["spawn", activeDevice, executable, resolve(container, "Documents"), String(await unusedPort())]);
      }
      const categories = full ? ["large", "accessibility-extra-extra-extra-large"] : ["large"];
      for (const category of categories) {
        await sim(["ui", activeDevice, "content_size", category]);
        const tests = !matrixOnly && index === 0 && category === "large" ? [...smokeTests, ...(full ? ["testRuleLibraryUpdate", "testVisualMatrix"] : [])] : ["testVisualMatrix"];
        const resultPath = resolve(output, `${index}-${category}.xcresult`);
        const result = await run("xcodebuild", ["test-without-building", "-xctestrun", testRun, "-destination", `platform=iOS Simulator,id=${activeDevice}`, "-parallel-testing-enabled", "NO", "-resultBundlePath", resultPath, ...tests.map((test) => `-only-testing:VoyaVPNUITests/VoyaVPNUITests/${test}`)], { label: `tests-${index}-${category}`, allowFailure: true });
        results.push({ device: type.name, category, tests, passed: result.code === 0, resultPath });
        await run("xcrun", ["xcresulttool", "export", "attachments", "--path", resultPath, "--output-path", resolve(output, `attachments-${index}-${category}`)], { allowFailure: true });
        await sim(["spawn", activeDevice, "log", "show", "--last", "1h", "--style", "compact", "--predicate", 'process == "VoyaVPN"'], { allowFailure: true, label: `native-log-${index}-${category}` });
        // A remaining run directory signals failed stop/cancellation cleanup.
        const probeRoot = resolve(container, "Documents/PTest");
        if (existsSync(probeRoot) && readdirSync(probeRoot).length) throw new Error(`Probe directories leaked: ${probeRoot}`);
        // Independent size/text cases still provide useful evidence after a
        // UI assertion fails. Keep collecting them; the final exit remains red.
        if (result.code !== 0) console.error(`XCTest failed for ${type.name}/${category}; continuing independent cases.`);
      }
      core.kill("SIGTERM");
      await fixtures.close(); fixtures = undefined;
      await sim(["shutdown", activeDevice]);
    }
    if (results.some((result) => !result.passed)) throw new Error("One or more XCTest cases failed; see the complete results and attachments.");
    writeFileSync(resolve(output, "summary.json"), JSON.stringify({ status: "passed", scope: matrixOnly ? "display-matrix" : full ? "full" : "smoke", results, deviceOnly: "VPN authorization/data plane, kill switch and network transitions require a physical iPhone" }, null, 2));
    console.log(`PASS: ${output}`);
  } catch (error) {
    writeFileSync(resolve(output, "summary.json"), JSON.stringify({ status: "failed", classification: phase, error: String(error), results }, null, 2));
    throw error;
  } finally {
    cleanupSignal();
    if (fixtures) await fixtures.close();
    for (const device of devices) {
      await sim(["shutdown", device], { allowFailure: true });
      await sim(["delete", device], { allowFailure: true });
    }
    process.removeListener("SIGINT", cleanupSignal);
    process.removeListener("SIGTERM", cleanupSignal);
  }
}

if (isCliEntrypoint(import.meta.url)) main().catch((error) => { console.error(error); process.exitCode = 1; });
