import { isCliEntrypoint, repoRootFromScript } from "../lib/common.mjs";
import { writeStableUpdaterOverlay } from "../tauri/stable-updater-config.mjs";

const commandLoaders = {
  artifacts: () => import("./commands/artifacts.mjs"),
  index: () => import("./commands/index.mjs"),
  "core-assets": () => import("./commands/core-assets.mjs"),
  updater: () => import("./commands/updater.mjs"),
  readiness: () => import("./commands/readiness.mjs"),
  "check-hosts": () => import("./commands/check-hosts.mjs"),
};

export const releaseCommandNames = [...Object.keys(commandLoaders), "updater-config"];

export function printHelp(stream = process.stdout) {
  stream.write(`Usage: pnpm release -- <command> [options]

Commands:
  artifacts        Normalize bundle artifacts and write artifact-manifest.json
  index            Generate the CDN release index and evidence
  core-assets      Generate the core asset manifest and evidence
  updater          Generate updater metadata and evidence
  readiness        Run dry-run or stable release readiness checks
  check-hosts      Validate stable release host and signing environment inputs
  updater-config   Generate the stable Tauri updater config overlay
`);
}

function isUsageError(error) {
  const message = error instanceof Error ? error.message : String(error);
  return /^(Unknown argument|Unknown release command)| requires a value| must be provided| is required/.test(message);
}

export async function runReleaseCli(
  argv,
  {
    stdout = process.stdout,
    stderr = process.stderr,
    env = process.env,
    repoRoot = repoRootFromScript(import.meta.url),
    loaders = commandLoaders,
  } = {},
) {
  const normalizedArgv = argv[0] === "--" ? argv.slice(1) : argv;
  const [command, ...commandArgs] = normalizedArgv;
  if (!command || command === "--help" || command === "-h") {
    printHelp(stdout);
    return 0;
  }

  try {
    if (command === "updater-config") {
      if (commandArgs.includes("--help") || commandArgs.includes("-h")) {
        stdout.write("Usage: pnpm release -- updater-config\n");
        return 0;
      }
      if (commandArgs.length > 0) {
        throw new Error(`Unknown argument: ${commandArgs[0]}`);
      }
      const overlayPath = writeStableUpdaterOverlay({ repoRoot, env });
      stdout.write(`Wrote stable Tauri updater config overlay: ${overlayPath}\n`);
      return 0;
    }

    const load = loaders[command];
    if (!load) {
      throw new Error(`Unknown release command: ${command}`);
    }
    const module = await load();
    await module.main(commandArgs);
    return 0;
  } catch (error) {
    stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    return isUsageError(error) ? 2 : 1;
  }
}

export async function main(argv = process.argv.slice(2)) {
  process.exitCode = await runReleaseCli(argv);
}

if (isCliEntrypoint(import.meta.url)) {
  await main();
}
