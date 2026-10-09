import { isCliEntrypoint, truthy } from "../lib/common.mjs";
import { parseInstallArgs, runSeedInstall } from "./install-entry.mjs";
import { installSingBoxCore } from "./sing-box-installer.mjs";

if (isCliEntrypoint(import.meta.url)) {
  await runSeedInstall({
    label: "sing-box",
    retry: "Run `node scripts/core/install-sing-box.mjs --force` to retry manually.",
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
