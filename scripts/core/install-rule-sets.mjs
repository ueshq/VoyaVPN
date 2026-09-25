import { isCliEntrypoint, truthy } from "../lib/common.mjs";
import { runSeedInstall } from "./install-entry.mjs";
import { installRuleSetSeeds } from "./rule-sets-installer.mjs";
import { parseInstallArgs } from "./sing-box-installer.mjs";

if (isCliEntrypoint(import.meta.url)) {
  await runSeedInstall({
    label: "rule-set",
    retry: "Run `pnpm core:rule-sets:install` to retry manually; package builds stage them too.",
    install: async ({ postinstall, repoRoot }) => {
      const result = await installRuleSetSeeds({
        force: parseInstallArgs(process.argv.slice(2)).forceInstall || truthy(process.env.VOYAVPN_FORCE_RULE_SETS_FETCH),
        postinstall,
        repoRoot,
      });
      return result.status === "staged" ? `rule-set seeds staged: ${result.dir}` : null;
    },
  });
}
