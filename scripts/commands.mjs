/**
 * Every workspace command, keyed by the words typed after `vp run`:
 * `vp run build mac local` is the entry "build mac local". The root
 * package.json holds one script per first word, each pointing at
 * scripts/run.mjs, which resolves the rest here.
 *
 * A command is a list of steps run in order, stopping at the first failure;
 * arguments typed after the command go to the last step. Steps name only
 * `node`, `cargo` and `vp`, which are native executables on every platform, so
 * nothing here needs a shell: a node_modules binary runs through `vp exec`.
 */
const node = (...args) => ["node", args];
const cargo = (...args) => ["cargo", args];
const vp = (...args) => ["vp", args];

/**
 * The checks that read only tracked sources, and so run as cached tasks
 * (`run.tasks` in vite.config.ts): an unchanged tree replays the last result.
 * Task names cannot hold a space, hence the hyphenated twin of each command.
 */
export const cachedTasks = {
  "check architecture": "check-architecture",
  "check lockfile": "check-lockfile",
  "check i18n": "check-i18n",
};

const cached = (command) => ({ steps: [vp("run", cachedTasks[command])] });

export const commands = {
  "dev web": { steps: [vp("dev")] },
  "dev ios": { steps: [node("scripts/native/mobile/dev.mjs", "ios")] },
  "dev android": { steps: [node("scripts/native/mobile/dev.mjs", "android")] },

  "build mac": { steps: [node("scripts/native/macos/build-app.mjs")] },
  "build mac local": {
    env: { VOYAVPN_SKIP_NOTARIZATION: "1" },
    steps: [node("scripts/native/macos/build-app.mjs")],
  },
  "build mac appstore": {
    env: { VOYAVPN_MAC_APP_STORE: "1" },
    steps: [node("scripts/native/macos/build-app.mjs")],
  },
  "build ios appstore": { steps: [node("scripts/native/mobile/build-ios-appstore.mjs")] },
  "build android": { steps: [node("scripts/native/mobile/build-android.mjs")] },
  "build windows local": { steps: [node("scripts/native/windows/build-app.mjs")] },

  "check architecture": cached("check architecture"),
  "check lockfile": cached("check lockfile"),
  "check i18n": cached("check i18n"),
  "check bindings": { steps: [node("scripts/quality/bindings.mjs", "--check")] },
  // Not a cached task: knip writes into the tree it reads, which the task
  // cache refuses to fingerprint.
  "check dead-code": {
    steps: [
      vp("exec", "knip", "--treat-config-hints-as-errors"),
      vp("exec", "knip", "--strict", "--no-config-hints", "--config", "knip.production.jsonc"),
    ],
  },
  "check sing-box": { steps: [node("scripts/quality/sing-box.mjs")] },
  "check rust fmt": { steps: [cargo("fmt", "--all", "--check")] },
  "check rust clippy": { steps: [cargo("clippy", "--workspace", "--all-targets", "--", "-D", "warnings")] },
  "check rust deps": { steps: [cargo("machete")] },
  "check rust test": { steps: [node("scripts/quality/rust-tests.mjs")] },
  "check frontend bundle": { steps: [vp("build"), node("scripts/quality/frontend-bundle.mjs")] },
  "check frontend coverage": { steps: [vp("test", "--coverage"), node("scripts/quality/frontend-coverage.mjs")] },
  "check frontend smoke mock": { steps: [vp("exec", "--filter", "@voya/desktop", "playwright", "test")] },
  "check desktop smoke": {
    steps: [vp("run", "tauri", "build", "--debug", "--no-bundle"), node("apps/desktop/e2e/desktop-smoke.mjs")],
  },
  "check native macos bridge": { steps: [node("scripts/native/macos/test-bridge.mjs")] },
  "check mobile test": { steps: [vp("run", "--filter", "@voya/mobile", "test")] },
  "check mobile bundle": { steps: [node("scripts/quality/mobile-bundle.mjs")] },
  "check mobile swift": { steps: [node("scripts/quality/mobile-swift.mjs")] },
  "check mobile ios assets": { steps: [node("scripts/native/mobile/ios-bundle-checks.mjs")] },
  "check mobile ios smoke": { steps: [node("scripts/native/mobile/ios-smoke.mjs")] },

  "generate bindings": { steps: [node("scripts/quality/bindings.mjs")] },
  "size report": { steps: [node("scripts/quality/package-size.mjs")] },
  "bench rust": {
    steps: [
      cargo("bench", "-p", "voya-core", "--bench", "config_generation"),
      cargo("bench", "-p", "voya-app", "--bench", "subscription_import"),
    ],
  },
  "verify local": { steps: [node("scripts/quality/verify-local.mjs")] },

  "core sing-box build": { steps: [node("scripts/core/sing-box-source-seed.mjs")] },
  "core rule-sets install": { steps: [node("scripts/core/install-rule-sets.mjs", "--force")] },

  "native macos libbox": { steps: [node("scripts/native/macos/build-libbox.mjs")] },
  "native macos tunnel": { steps: [node("scripts/native/macos/build-tunnel.mjs")] },
  "native macos tunnel verify": { steps: [node("scripts/native/macos/verify-tunnel.mjs")] },
  "native macos app sign": { steps: [node("scripts/native/macos/sign-app.mjs")] },
  "native macos app notarize": { steps: [node("scripts/native/macos/notarize-app.mjs")] },
  "native macos ne doctor": { steps: [node("scripts/native/macos/ne-doctor.mjs")] },
  "native macos preflight": { steps: [node("scripts/native/macos/preflight.mjs")] },
  "native macos dmg": { steps: [node("scripts/native/macos/create-dmg.mjs")] },
  "native macos pkg": { steps: [node("scripts/native/macos/create-pkg.mjs")] },
  // build | install | uninstall | status: the script reads the word itself.
  "native windows tunnel": { steps: [node("scripts/native/windows/tunnel-service.mjs")] },
  "native mobile rust ios": { steps: [node("scripts/native/mobile/build-rust-ios.mjs")] },
  "native mobile rust android": { steps: [node("scripts/native/mobile/build-rust-android.mjs")] },
  "native mobile libbox ios": { steps: [node("scripts/native/mobile/build-libbox-ios.mjs")] },
  "native mobile libbox android": { steps: [node("scripts/native/mobile/build-libbox-android.mjs")] },
  "native mobile ios project": { steps: [node("scripts/native/mobile/ios-project.mjs")] },
  "native mobile ios verify-imports": { steps: [node("scripts/native/mobile/ios-verify-imports.mjs")] },
  "native mobile ios icons": { steps: [node("scripts/native/mobile/ios-icons.mjs")] },
};

/** The first words: one root package.json script each. */
export const groups = [...new Set(Object.keys(commands).map((name) => name.split(" ")[0]))];

/**
 * Splits what was typed into a command and the arguments left for it. The
 * longest name wins, so `native macos tunnel verify` is its own command and
 * not `native macos tunnel` given the argument `verify`. A leading `--` in the
 * remainder is dropped: it only separated the command from its arguments.
 */
export function resolveCommand(words, table = commands) {
  const flagAt = words.findIndex((word) => word.startsWith("-"));
  const nameWords = flagAt === -1 ? words : words.slice(0, flagAt);
  for (let length = nameWords.length; length > 0; length -= 1) {
    const name = nameWords.slice(0, length).join(" ");
    if (Object.hasOwn(table, name)) {
      const rest = words.slice(length);
      return { name, command: table[name], args: rest[0] === "--" ? rest.slice(1) : rest };
    }
  }
  return null;
}

/** The commands that start with the words typed, for the usage message. */
export function commandsUnder(words, table = commands) {
  const names = Object.keys(table);
  for (let length = words.length; length > 0; length -= 1) {
    const prefix = `${words.slice(0, length).join(" ")} `;
    const matches = names.filter((name) => name.startsWith(prefix));
    if (matches.length > 0) return matches;
  }
  return names;
}
