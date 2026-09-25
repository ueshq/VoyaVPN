import { isCliEntrypoint, truthy } from "../lib/common.mjs";
import { runSeedInstall } from "./install-entry.mjs";
import { installSingBoxCore, parseInstallArgs } from "./sing-box-installer.mjs";

if (isCliEntrypoint(import.meta.url)) {
  await runSeedInstall({
    label: "sing-box",
    retry: "Run `pnpm core:sing-box:install` to retry manually.",
    install: async ({ postinstall, repoRoot }) => {
      const args = parseInstallArgs(process.argv.slice(2));
      const result = await installSingBoxCore({
        forceFetch: args.forceFetch || truthy(process.env.VOYAVPN_FORCE_SING_BOX_FETCH),
        forceInstall: args.forceInstall,
        postinstall,
        repoRoot,
      });
      return result.status === "installed" ? `sing-box installed: ${result.executable}` : null;
    },
  });
}
