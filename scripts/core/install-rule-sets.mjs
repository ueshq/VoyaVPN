import { isCliEntrypoint, truthy } from "../lib/common.mjs";
import { parseInstallArgs, runSeedInstall } from "./install-entry.mjs";
import { installRuleSetSeeds } from "./rule-sets-installer.mjs";

if (isCliEntrypoint(import.meta.url)) {
  await runSeedInstall({
    label: "rule-set",
    retry: "Run `vp run core:rule-sets:install` to retry manually; package builds stage them too.",
    install: async ({ postinstall, repoRoot }) => {
      const { forceFetch, forceInstall } = parseInstallArgs(process.argv.slice(2));
      const result = await installRuleSetSeeds({
        // Staging a rule set is fetching it, so both flags mean the same here.
        force: forceInstall || forceFetch || truthy(process.env.VOYAVPN_FORCE_RULE_SETS_FETCH),
        postinstall,
        repoRoot,
      });
      return result.status === "staged" ? `rule-set seeds staged: ${result.dir}` : null;
    },
  });
}
