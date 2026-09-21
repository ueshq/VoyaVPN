import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import {
  capture,
  isCliEntrypoint,
  repoRootFromScript,
  requireDarwin,
  run,
} from "../../lib/common.mjs";

/**
 * Wires the iOS Xcode project up to everything this repo builds for it.
 *
 * The work itself is `ios-project.rb`, because editing a `.pbxproj` by hand is
 * how project files get corrupted and the `xcodeproj` gem is the only sane way
 * to do it. This wrapper exists to find a Ruby that can `require` that gem and
 * to give the script a place in `pnpm run`.
 *
 * Re-runnable on purpose: `pod install` and a React Native upgrade both rewrite
 * parts of the project, and this is what puts our half back.
 */
const SCRIPT = "ios-project.rb";

/**
 * A Ruby that can see the `xcodeproj` gem, as `[binary, env]`.
 *
 * The system Ruby cannot: macOS ships 2.6 with no gems worth having. CocoaPods
 * bundles the gem in its own gem home, so the fallback reads that home out of
 * the `pod` wrapper rather than hardcoding a Homebrew Cellar version, which
 * changes with every CocoaPods release.
 */
export function rubyWithXcodeproj({ captureCommand = capture } = {}) {
  const probe = (binary, env) =>
    captureCommand(binary, ["-e", "require 'xcodeproj'"], { env: { ...process.env, ...env } })
      .status === 0;

  if (probe("ruby", {})) return { binary: "ruby", env: {} };

  // `which`, not `command -v`: the latter is a shell builtin and there is no
  // shell here.
  const podPath = captureCommand("which", ["pod"]).stdout?.trim();
  const wrapper = podPath && existsSync(podPath) ? readFileSync(podPath, "utf8") : "";
  const gemHome = wrapper.match(/GEM_HOME="([^"]+)"/u)?.[1];
  if (!gemHome) {
    throw new Error(missingGemMessage());
  }

  // The gem home's own `pod` names the Ruby it was installed for; the system
  // one is too old to load it.
  const shebang = existsSync(resolve(gemHome, "bin/pod"))
    ? readFileSync(resolve(gemHome, "bin/pod"), "utf8").split("\n")[0]
    : "";
  const binary = shebang.startsWith("#!") ? shebang.slice(2).trim() : "ruby";
  // `GEM_HOME` only, exactly as CocoaPods' own wrapper does it. Pinning
  // `GEM_PATH` to the same directory would cut the Ruby off from its default
  // gems, and `xcodeproj` needs `rexml` from those.
  const env = { GEM_HOME: gemHome };
  if (!probe(binary, env)) {
    throw new Error(missingGemMessage());
  }

  return { binary, env };
}

function missingGemMessage() {
  return [
    "Could not find a Ruby that can load the `xcodeproj` gem.",
    "It ships with CocoaPods, which this project already needs:",
    "  brew install cocoapods",
    "Or install the gem on its own:",
    "  gem install xcodeproj",
  ].join("\n");
}

export function wireIosProject() {
  requireDarwin("The iOS project can only be wired up on macOS.");
  const repoRoot = repoRootFromScript(import.meta.url);
  const { binary, env } = rubyWithXcodeproj();

  run(binary, [resolve(repoRoot, "scripts/native/mobile", SCRIPT)], {
    cwd: repoRoot,
    env: { ...process.env, ...env },
  });
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    wireIosProject();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
