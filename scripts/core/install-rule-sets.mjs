import { isCliEntrypoint, repoRootFromScript, truthy } from "../lib/common.mjs";
import { installRuleSetSeeds } from "./rule-sets-installer.mjs";

async function main() {
  const postinstall = process.env.npm_lifecycle_event === "postinstall";

  try {
    const result = await installRuleSetSeeds({
      force: process.argv.includes("--force") || truthy(process.env.VOYAVPN_FORCE_RULE_SETS_FETCH),
      postinstall,
      repoRoot: repoRootFromScript(import.meta.url),
    });
    if (result.status === "staged") {
      console.log(`rule-set seeds staged: ${result.dir}`);
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (postinstall) {
      console.warn(`rule-set postinstall did not complete: ${message}`);
      console.warn("Run `pnpm core:rule-sets:install` to retry manually; package builds stage them too.");
      return;
    }

    console.error(message);
    process.exit(1);
  }
}

if (isCliEntrypoint(import.meta.url)) {
  await main();
}
