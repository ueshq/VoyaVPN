import {
  installSingBoxCore,
  isCliEntrypoint,
  parseInstallArgs,
  repoRootFromScript,
  truthy,
} from "./sing-box-core-installer.mjs";

async function main() {
  const args = parseInstallArgs(process.argv.slice(2));
  const postinstall = process.env.npm_lifecycle_event === "postinstall";

  try {
    const result = await installSingBoxCore({
      forceFetch: args.forceFetch || truthy(process.env.VOYAVPN_FORCE_SING_BOX_FETCH),
      forceInstall: args.forceInstall,
      postinstall,
      repoRoot: repoRootFromScript(import.meta.url),
    });
    if (result.status === "installed") {
      console.log(`sing-box installed: ${result.executable}`);
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (postinstall) {
      console.warn(`sing-box postinstall did not complete: ${message}`);
      console.warn("Run `pnpm core:sing-box:install` to retry manually.");
      return;
    }

    console.error(message);
    process.exit(1);
  }
}

if (isCliEntrypoint(import.meta.url)) {
  await main();
}
