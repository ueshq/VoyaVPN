import { existsSync } from "node:fs";
import { join } from "node:path";
import { capture, captureSpawned, checkedCapture, requireDarwin, runCli, sleepSync } from "../../lib/common.mjs";
import { parseArgs } from "../../lib/args.mjs";

// Drives an installed VoyaVPN.app through the window and menu bar paths that
// only a real, sandboxed bundle exercises: App Review stopped build 407 because
// the menu bar item appeared to do nothing, and none of it showed in a test.
// It clicks through System Events, so the terminal running it needs
// Accessibility permission, and it moves the app's window while it runs.

const defaultAppBundle = "/Applications/VoyaVPN.app";
const executableName = "voyavpn";
const pollIntervalMs = 200;

const argSpec = {
  "--app": { key: "app" },
};

function printHelp() {
  console.log(`Usage: vp run native macos window smoke [--app ${defaultAppBundle}]

Checks, on a running or freshly launched VoyaVPN.app:
  - closing the window and choosing the menu bar item shows it again
  - the menu bar item acts on its first click after the window is minimized
  - hiding a full-screen window leaves full screen first
  - a second launch (login item or plain) leaves one copy and one menu bar icon
  - Quit ends the process (only when this script launched the app)

Needs "Closing the window" set to keep VoyaVPN in the menu bar (the default),
and Accessibility permission for the terminal. An app that was already running
is left running, with its window shown.`);
}

function bundleIdentifier(app) {
  return checkedCapture("/usr/libexec/PlistBuddy", [
    "-c",
    "Print :CFBundleIdentifier",
    join(app, "Contents", "Info.plist"),
  ]).stdout.trim();
}

function runningPids(executable) {
  const listing = checkedCapture("ps", ["-axo", "pid=,command="]).stdout;
  return listing
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => {
      const command = line.slice(line.indexOf(" ") + 1);
      return command === executable || command.startsWith(`${executable} `);
    })
    .map((line) => Number(line.slice(0, line.indexOf(" "))));
}

/** One System Events statement addressed to the app's process. */
function events(bundleId, statement) {
  const script = `tell application "System Events" to tell (first process whose bundle identifier is "${bundleId}") to ${statement}`;
  const result = captureSpawned("osascript", ["-e", script], { timeout: 30_000 });
  if (result.status !== 0) {
    const detail = result.stderr.trim();
    const hint = /not allowed assistive access|-1719|-25211/.test(detail)
      ? "\nGrant the terminal Accessibility permission in System Settings > Privacy & Security."
      : "";
    throw new Error(`System Events refused: ${statement}\n${detail}${hint}`);
  }
  return result.stdout.trim();
}

function waitFor(description, condition, timeoutMs = 8000) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (condition()) return;
    if (Date.now() >= deadline) {
      throw new Error(`Timed out waiting for: ${description}`);
    }
    sleepSync(pollIntervalMs);
  }
}

function smoke(app) {
  const executable = join(app, "Contents", "MacOS", executableName);
  if (!existsSync(executable)) {
    throw new Error(`No VoyaVPN executable at ${executable}`);
  }
  const bundleId = bundleIdentifier(app);
  const ask = (statement) => events(bundleId, statement);
  const windowCount = () => Number(ask("return count of windows"));
  const windowFlag = (attribute) => ask(`return value of attribute "${attribute}" of window 1`) === "true";
  const trayIcons = () => Number(ask("return count of menu bar items of menu bar 2"));
  // The menu is [connect, -, traffic mode, nodes, -, show/hide, -, quit] in the
  // app's own language, so its items are found by position, not by title.
  const trayItems = () => Number(ask("return count of menu items of menu 1 of menu bar item 1 of menu bar 2"));
  const clickTray = (fromEnd) =>
    ask(`click menu item ${trayItems() - fromEnd} of menu 1 of menu bar item 1 of menu bar 2`);
  const toggleWindow = () => clickTray(2);
  const passed = (line) => console.log(`ok  ${line}`);

  const launchedHere = runningPids(executable).length === 0;
  if (launchedHere) {
    checkedCapture("open", [app]);
  }
  waitFor("VoyaVPN to be running", () => runningPids(executable).length === 1, 20_000);
  waitFor(
    "the menu bar icon",
    () => {
      try {
        return trayIcons() === 1;
      } catch {
        // The process is not registered with System Events until AppKit is up.
        return false;
      }
    },
    20_000,
  );
  if (windowCount() === 0) toggleWindow();
  waitFor("the main window", () => windowCount() === 1);

  ask('click (first button of window 1 whose subrole is "AXCloseButton")');
  try {
    waitFor("the closed window to hide", () => runningPids(executable).length === 1 && windowCount() === 0);
  } catch {
    throw new Error(
      'Closing the window did not hide it into the menu bar. Set Settings > "Closing the window" to keep VoyaVPN running, then run again.',
    );
  }
  toggleWindow();
  waitFor("the menu bar item to show the closed window", () => windowCount() === 1);
  passed("closed window comes back from the menu bar");

  ask('set value of attribute "AXMinimized" of window 1 to true');
  waitFor("the window to minimize", () => windowFlag("AXMinimized"));
  // The menu follows the window through a focus event; give it its rebuild.
  sleepSync(1000);
  toggleWindow();
  waitFor("the first menu bar click to restore the minimized window", () => !windowFlag("AXMinimized"));
  passed("minimized window comes back on the first click");

  ask('set value of attribute "AXFullScreen" of window 1 to true');
  waitFor("the window to enter full screen", () => windowFlag("AXFullScreen"));
  sleepSync(1500);
  toggleWindow();
  waitFor("the full-screen window to hide", () => windowCount() === 0, 12_000);
  toggleWindow();
  waitFor("the window to return", () => windowCount() === 1);
  if (windowFlag("AXFullScreen")) {
    throw new Error("The window was hidden while still full screen, which leaves an empty Space behind.");
  }
  passed("full-screen window leaves full screen before it hides");

  // What the login agent does: start the executable directly.
  const loginLaunch = capture(executable, ["--autostart"], { timeout: 15_000, stdio: "ignore" });
  if (loginLaunch.error || loginLaunch.status !== 0) {
    throw new Error("A login launch beside the running app did not exit on its own: two copies can run.");
  }
  if (runningPids(executable).length !== 1 || trayIcons() !== 1) {
    throw new Error("A login launch left a second copy or a second menu bar icon.");
  }
  passed("login launch beside a running copy exits");

  toggleWindow();
  waitFor("the window to hide", () => windowCount() === 0);
  const plainLaunch = capture(executable, [], { timeout: 15_000, stdio: "ignore" });
  if (plainLaunch.error || plainLaunch.status !== 0) {
    throw new Error("A second plain launch did not exit on its own: two copies can run.");
  }
  waitFor("a second launch to show the running copy's window", () => windowCount() === 1);
  passed("second launch shows the running copy's window");

  if (launchedHere) {
    clickTray(0);
    waitFor("Quit to end the process", () => runningPids(executable).length === 0, 60_000);
    passed("Quit ends the process");
  }
}

runCli(() => {
  const options = parseArgs(process.argv.slice(2), argSpec, { app: defaultAppBundle });
  if (options.help) {
    printHelp();
    return;
  }
  requireDarwin("The window smoke drives a macOS app bundle.");
  smoke(options.app);
  console.log("macOS window smoke passed.");
});
