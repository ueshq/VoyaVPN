import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { capture, isCliEntrypoint, repoRootFromScript, requireDarwin, run } from "../../lib/common.mjs";
import { parseArgs } from "../../lib/args.mjs";
import { defaultIsProcessRunning, voyaRuntimeExecutables } from "./local-runtime.mjs";
import {
  packetTunnelBundleIdentifier as providerBundleId,
  packetTunnelLayout,
} from "./tunnel-layout.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const defaultAppBundle = "/Applications/VoyaVPN.app";
const repoReleaseAppBundle = resolve(repoRoot, "target", "release", "bundle", "macos", "VoyaVPN.app");

const argSpec = {
  "--app": { key: "app" },
  "--dev": { key: "dev", value: true },
  "--fix": { key: "fix", value: true },
  "--yes|-y": { key: "yes", value: true },
};

function parseDoctorArgs(argv) {
  return parseArgs(argv, argSpec, {
    app: defaultAppBundle,
    dev: false,
    fix: false,
    yes: false,
  });
}

function printHelp() {
  console.log(`Usage: pnpm native:macos:ne:doctor [--fix] [--app /Applications/VoyaVPN.app] [--dev]

Checks macOS PacketTunnel registrations for ${providerBundleId}.

Options:
  --fix        Unregister stale appex entries, refresh the selected app, then re-check.
               Refuses to run while VoyaVPN is running, or when the selected app
               has no PacketTunnel provider.
  --app PATH   Legal VoyaVPN.app path. Defaults to /Applications/VoyaVPN.app.
  --dev        Also allow target/release/bundle/macos/VoyaVPN.app for local release builds.
  --yes, -y    Confirm removing more than one registration in a single --fix.`);
}

function throwIfSpawnFailed(result) {
  if (result.error) {
    throw result.error;
  }
  return result;
}

function appBundleFromInput(path) {
  const resolved = resolve(path);
  if (resolved.endsWith(".appex")) {
    const appRoot = appBundleForAppex(resolved);
    if (!appRoot) {
      throw new Error(`Unable to resolve containing .app for appex: ${resolved}`);
    }
    return appRoot;
  }
  return resolved;
}

function appexForApp(appBundle) {
  return packetTunnelLayout(resolve(appBundle, "Contents"), "app-store").bundle;
}

function sysexForApp(appBundle) {
  return packetTunnelLayout(resolve(appBundle, "Contents"), "developer-id").bundle;
}

function selectedProviderForApp(appBundle) {
  const sysex = sysexForApp(appBundle);
  if (existsSync(sysex)) {
    return { mode: "system-extension", path: sysex };
  }
  return { mode: "app-extension", path: appexForApp(appBundle) };
}

function appBundleForAppex(appexPath) {
  const marker = ".app/";
  const markerIndex = appexPath.indexOf(marker);
  if (markerIndex === -1) {
    return null;
  }
  return appexPath.slice(0, markerIndex + ".app".length);
}

function normalizePath(path) {
  return resolve(path.replace(/^file:\/\//, ""));
}

export function parsePluginkitMatches(output) {
  const matches = [];
  const seen = new Set();
  const escapedId = providerBundleId.replaceAll(".", "\\.");
  // Bundle paths may contain spaces (`~/My Builds/VoyaVPN.app/...`), so the run
  // cannot exclude whitespace. Instead the path must start at a delimiter —
  // line start, whitespace, `=`, a quote, `(` or `,` — and is matched lazily up
  // to the provider's `.appex`, which keeps several paths on one line separable.
  // The previous `[^\s"']*` silently truncated a spaced path to its last
  // segment, and `--fix` then unregistered that nonexistent path.
  const pathPattern = new RegExp(`(?:^|[\\s="'(,])((?:file:)?/[^"'\\n]*?${escapedId}\\.appex)`, "g");

  for (const line of output.split(/\r?\n/)) {
    for (const match of line.matchAll(pathPattern)) {
      const path = normalizePath(match[1].replace(/[),;]+$/, ""));
      if (!seen.has(path)) {
        seen.add(path);
        matches.push(path);
      }
    }
  }

  return matches;
}

function collectPluginkitMatches() {
  const all = throwIfSpawnFailed(capture("/usr/bin/pluginkit", ["-mDvvv", "-i", providerBundleId], { cwd: repoRoot }));
  const active = throwIfSpawnFailed(capture("/usr/bin/pluginkit", ["-mAvvv", "-i", providerBundleId], { cwd: repoRoot }));
  if (all.status !== 0 && active.status !== 0) {
    throw new Error(
      `pluginkit query failed: ${all.stderr || all.stdout || active.stderr || active.stdout}`,
    );
  }

  return {
    active: parsePluginkitMatches(`${active.stdout ?? ""}\n${active.stderr ?? ""}`),
    all: parsePluginkitMatches(`${all.stdout ?? ""}\n${all.stderr ?? ""}`),
  };
}

function parseSystemExtensionMatches(output) {
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.includes(providerBundleId))
    .map((line) => {
      const state = line.match(/\[([^\]]+)\]\s*$/)?.[1] ?? "";
      return { line, state };
    });
}

function collectSystemExtensionMatches() {
  const result = throwIfSpawnFailed(capture("/usr/bin/systemextensionsctl", ["list"], { cwd: repoRoot }));
  if (result.status !== 0) {
    return [];
  }
  return parseSystemExtensionMatches(`${result.stdout ?? ""}\n${result.stderr ?? ""}`);
}

function buildLegalApps(options) {
  const legalApps = [appBundleFromInput(options.app)];
  if (options.dev) {
    const devApp = appBundleFromInput(repoReleaseAppBundle);
    if (!legalApps.includes(devApp)) {
      legalApps.push(devApp);
    }
  }
  return legalApps;
}

function classify(matches, legalApps) {
  const selectedProviders = legalApps.map(selectedProviderForApp);
  const mode = selectedProviders.some((entry) => entry.mode === "system-extension")
    ? "system-extension"
    : "app-extension";
  const legalProviderPaths = new Set(selectedProviders.map((entry) => entry.path));
  const activeSet = new Set(matches.active);
  const all = [...new Set([...matches.all, ...matches.active])].sort();

  const pluginkitEntries = all.map((path) => ({
    type: "appex",
    path,
    appBundle: appBundleForAppex(path),
    exists: existsSync(path),
    elected: activeSet.has(path),
    legal: mode === "app-extension" && legalProviderPaths.has(path),
    state: "",
  })).map((entry) => ({
    ...entry,
    stale: !entry.exists || !entry.legal,
  }));

  if (mode !== "system-extension") {
    return pluginkitEntries;
  }

  const systemEntries = collectSystemExtensionMatches().map((entry) => {
    const path = selectedProviders.find((provider) => provider.mode === "system-extension")?.path ?? sysexForApp(legalApps[0]);
    const activated = entry.state.includes("activated") && entry.state.includes("enabled");
    return {
      type: "systemextension",
      path,
      appBundle: legalApps[0],
      exists: existsSync(path),
      elected: activated,
      legal: existsSync(path),
      state: entry.state || entry.line,
      stale: !existsSync(path),
    };
  });

  if (systemEntries.length === 0) {
    const path = selectedProviders.find((provider) => provider.mode === "system-extension")?.path ?? sysexForApp(legalApps[0]);
    systemEntries.push({
      type: "systemextension",
      path,
      appBundle: legalApps[0],
      exists: existsSync(path),
      elected: false,
      legal: existsSync(path),
      state: "not registered",
      stale: !existsSync(path),
    });
  }

  return [...pluginkitEntries, ...systemEntries];
}

function tableRows(entries) {
  return entries.map((entry) => ({
    elected: entry.elected ? "yes" : "no",
    exists: entry.exists ? "yes" : "no",
    legal: entry.legal ? "yes" : "no",
    stale: entry.stale ? "yes" : "no",
    type: entry.type,
    state: entry.state,
    path: entry.path,
  }));
}

function printReport(entries, legalApps) {
  console.log(`macOS NetworkExtension provider: ${providerBundleId}`);
  console.log("Legal app bundle(s):");
  for (const app of legalApps) {
    console.log(`  ${app}`);
  }
  console.log("");

  if (entries.length === 0) {
    console.log("No PlugInKit registrations found for this provider id.");
  } else {
    console.table(tableRows(entries));
  }

  console.log("");
  console.log("VPN profile check:");
  console.log('  scutil --nc list | grep -F "VoyaVPN"');
  console.log("");
  console.log("The doctor does not remove VPN profiles. If stale registrations created a broken profile, remove it from System Settings > VPN.");
}

function health(entries) {
  const stale = entries.filter((entry) => entry.stale);
  const illegalActive = entries.filter((entry) => entry.elected && !entry.legal);
  const activeLegal = entries.filter((entry) => entry.elected && entry.legal && entry.exists);
  const systemExtensionEntries = entries.filter((entry) => entry.type === "systemextension");
  const readySystemExtensions = systemExtensionEntries.filter((entry) => entry.legal && entry.exists);
  const usesSystemExtension = systemExtensionEntries.length > 0;

  return {
    activeLegal,
    illegalActive,
    readySystemExtensions,
    stale,
    usesSystemExtension,
    ok: usesSystemExtension
      ? stale.length === 0 && illegalActive.length === 0 && readySystemExtensions.length >= 1
      : stale.length === 0 && illegalActive.length === 0 && activeLegal.length === 1,
  };
}

function unregisterEntry(entry) {
  if (entry.exists) {
    run("/usr/bin/pluginkit", ["-r", entry.path], { cwd: repoRoot });
    return;
  }

  if (entry.appBundle) {
    throwIfSpawnFailed(capture(
      "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
      ["-f", "-u", entry.appBundle],
      { cwd: repoRoot, stdio: "inherit" },
    ));
  }
}

function registerLegalApp(appBundle) {
  const provider = selectedProviderForApp(appBundle);
  if (!existsSync(provider.path)) {
    throw new Error(`Legal PacketTunnel provider is missing: ${provider.path}`);
  }

  run(
    "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
    ["-f", "-R", appBundle],
    { cwd: repoRoot },
  );
  if (provider.mode === "app-extension") {
    run("/usr/bin/pluginkit", ["-a", appBundle], { cwd: repoRoot });
    throwIfSpawnFailed(capture("/usr/bin/pluginkit", ["-a", provider.path], {
      cwd: repoRoot,
      stdio: "inherit",
    }));
  } else {
    console.log("System Extension activation is requested by the app when TUN starts; the doctor only refreshes LaunchServices.");
  }
}

/**
 * Decides what `--fix` may unregister, before anything is unregistered.
 *
 * `fix()` used to unregister every stale entry first and only then call
 * `registerLegalApp`, which throws when the selected app has no provider. A
 * mistyped `--app` makes *every* existing registration "not legal" and
 * therefore stale (classify()), so the machine was left with no PacketTunnel
 * registration at all before the error surfaced. PlugInKit registration is
 * machine-global state shared by every VoyaVPN.app on the box, so all three
 * preconditions are checked up front and nothing is torn down on failure.
 */
export function planPacketTunnelFix(
  entries,
  { providerPath, providerExists, runningExecutables = [], assumeYes = false } = {},
) {
  if (!providerExists) {
    throw new Error(
      `Legal PacketTunnel provider is missing: ${providerPath}. `
        + "Pass --app <path to a built VoyaVPN.app> (add --dev for the repo release bundle); "
        + "nothing was unregistered.",
    );
  }

  if (runningExecutables.length > 0) {
    throw new Error(
      `VoyaVPN is still running (${runningExecutables.join(", ")}). `
        + "Quit VoyaVPN before --fix so registrations are not torn down under a live provider; "
        + "nothing was unregistered.",
    );
  }

  const stale = entries.filter((entry) => entry.stale && entry.type === "appex");
  if (stale.length > 1 && !assumeYes) {
    throw new Error(
      `--fix would unregister ${stale.length} PacketTunnel registrations:\n`
        + stale.map((entry) => `  ${entry.path}`).join("\n")
        + "\nRe-run with --yes to confirm; nothing was unregistered.",
    );
  }

  return stale;
}

function runningVoyaExecutables(isProcessRunning = defaultIsProcessRunning) {
  return voyaRuntimeExecutables.filter((executable) => isProcessRunning(executable));
}

function fix(entries, legalApps, options) {
  const provider = selectedProviderForApp(legalApps[0]);
  const stale = planPacketTunnelFix(entries, {
    providerPath: provider.path,
    providerExists: existsSync(provider.path),
    runningExecutables: runningVoyaExecutables(),
    assumeYes: options.yes,
  });

  for (const entry of stale) {
    console.log(`Unregistering stale PacketTunnel provider: ${entry.path}`);
    unregisterEntry(entry);
  }

  registerLegalApp(legalApps[0]);
}

function main() {
  requireDarwin("macOS NetworkExtension doctor must run on macOS.");
  const options = parseDoctorArgs(process.argv.slice(2));
  if (options.help) {
    printHelp();
    return;
  }
  const legalApps = buildLegalApps(options);
  let entries = classify(collectPluginkitMatches(), legalApps);
  printReport(entries, legalApps);

  if (options.fix) {
    console.log("Applying PlugInKit registration repair...");
    fix(entries, legalApps, options);
    entries = classify(collectPluginkitMatches(), legalApps);
    console.log("");
    console.log("Post-fix report:");
    printReport(entries, legalApps);
  }

  const status = health(entries);
  if (!status.ok) {
    if (status.stale.length > 0) {
      console.error(`Found ${status.stale.length} stale or illegal PacketTunnel registration(s).`);
    }
    if (status.illegalActive.length > 0) {
      console.error("The elected PacketTunnel provider is not a legal VoyaVPN.app copy.");
    }
    if (status.usesSystemExtension && status.readySystemExtensions.length < 1) {
      console.error("Expected a legal staged PacketTunnel system extension, found none.");
    }
    if (!status.usesSystemExtension && status.activeLegal.length !== 1) {
      console.error(`Expected exactly one active legal provider, found ${status.activeLegal.length}.`);
    }
    console.error("Run `pnpm native:macos:ne:doctor --fix` after quitting VoyaVPN to repair registrations.");
    process.exit(1);
  }

  if (status.usesSystemExtension && status.activeLegal.length === 0) {
    console.log("PacketTunnel system extension is staged; the app will request activation when TUN starts.");
  } else {
    console.log("PacketTunnel registration health looks good.");
  }
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
