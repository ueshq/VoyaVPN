import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { repoRootFromScript } from "../lib/common.mjs";
import {
  criticalModules,
  evaluateCoverage,
  globalMinimums,
  runtimeModules,
  untestedModules,
} from "./frontend-coverage-policy.mjs";

const root = repoRootFromScript(import.meta.url);
const summary = JSON.parse(readFileSync(resolve(root, "coverage/coverage-summary.json"), "utf8"));

const { failures, warnings } = evaluateCoverage({
  total: summary.total,
  lookup: (relativePath) => summary[resolve(root, relativePath)],
});

for (const warning of warnings) console.warn(`! ${warning}`);

if (failures.length > 0) {
  console.error("\nFrontend coverage policy failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  console.error(
    "\nGlobal minimums and per-module floors live in scripts/quality/frontend-coverage-policy.mjs.",
  );
  process.exit(1);
}

console.log(
  `Frontend coverage policy passed: totals >= ${globalMinimums.lines}/${globalMinimums.functions}/${globalMinimums.branches}/${globalMinimums.statements}% ` +
    `(lines/functions/branches/statements), ${criticalModules.length} critical modules >= 80%, ` +
    `${runtimeModules.length} runtime modules above their floors, ${untestedModules.length} tracked as untested.`,
);
