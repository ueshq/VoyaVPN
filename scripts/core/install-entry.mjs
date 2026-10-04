import { parseArgs } from "../lib/args.mjs";
import { repoRootFromScript } from "../lib/common.mjs";

/** The flags both seed installers take on the command line. */
export function parseInstallArgs(argv) {
  return parseArgs(
    argv,
    {
      "--force": { key: "forceInstall", value: true },
      "--force-fetch": { key: "forceFetch", value: true },
    },
    { forceFetch: false, forceInstall: false },
  );
}

/**
 * The CLI wrapper shared by the two seed installers that `postinstall` runs.
 *
 * `install` receives whether this is a postinstall run and the repo root and
 * returns a line to print, or nothing. A postinstall failure is a warning that
 * names the manual retry command, because `pnpm install` must not fail on an
 * offline machine; a direct run of the same script exits non-zero.
 */
export async function runSeedInstall({ label, retry, install, env = process.env }) {
  const postinstall = env.npm_lifecycle_event === "postinstall";

  try {
    const message = await install({ postinstall, repoRoot: repoRootFromScript(import.meta.url) });
    if (message) {
      console.log(message);
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (postinstall) {
      console.warn(`${label} postinstall did not complete: ${message}`);
      console.warn(retry);
      return;
    }

    console.error(message);
    process.exit(1);
  }
}
