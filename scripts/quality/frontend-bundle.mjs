import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { isCliEntrypoint, repoRootFromScript } from "../lib/common.mjs";

/**
 * Bundle budgets are a ratchet, not a ceiling nobody can reach: each budget
 * sits roughly 1.3-1.5x the size the chunk had when it was set, so an
 * accidental dependency import is caught while ordinary feature work is not.
 * **Lower the budget when a chunk shrinks.** Sizes recorded 2026-09-07:
 * index 215.5 KiB, vendor-qr 456.8 KiB, vendor-data 241.0 KiB,
 * vendor-react 185.2 KiB, vendor-radix 154.8 KiB, server-table 72.5 KiB,
 * settings-screen 40.1 KiB; total emitted JS 1517 KiB.
 */
const budgets = [
  { label: "application entry", maxKiB: 300, prefix: "index-" },
  { label: "profiles screen", maxKiB: 100, prefix: "server-table-" },
  { label: "settings screen", maxKiB: 70, prefix: "settings-screen-" },
  { label: "QR decoder", maxKiB: 500, prefix: "vendor-qr-" },
  { label: "data vendor chunk", maxKiB: 320, prefix: "vendor-data-" },
  { label: "React vendor chunk", maxKiB: 240, prefix: "vendor-react-" },
  { label: "Radix vendor chunk", maxKiB: 210, prefix: "vendor-radix-" },
];

const totalBudgetKiB = 1900;

export function checkBundleBudgets(assets, { budgets: budgetList = budgets, totalKiB = totalBudgetKiB } = {}) {
  const failures = [];
  const report = [];

  for (const budget of budgetList) {
    const asset = assets.find(({ name }) => name.startsWith(budget.prefix));
    if (!asset) {
      failures.push(`${budget.label} bundle (${budget.prefix}*.js) was not generated`);
      continue;
    }

    const sizeKiB = asset.bytes / 1024;
    if (sizeKiB > budget.maxKiB) {
      failures.push(`${budget.label} bundle is ${sizeKiB.toFixed(1)} KiB; budget is ${budget.maxKiB} KiB`);
      continue;
    }
    report.push(`${budget.label}: ${sizeKiB.toFixed(1)} KiB / ${budget.maxKiB} KiB`);
  }

  const totalActualKiB = assets.reduce((sum, asset) => sum + asset.bytes, 0) / 1024;
  if (totalActualKiB > totalKiB) {
    failures.push(`total emitted JavaScript is ${totalActualKiB.toFixed(1)} KiB; budget is ${totalKiB} KiB`);
  } else {
    report.push(`total emitted JavaScript: ${totalActualKiB.toFixed(1)} KiB / ${totalKiB} KiB`);
  }

  return { failures, report };
}

if (isCliEntrypoint(import.meta.url)) {
  const assetsDir = join(repoRootFromScript(import.meta.url), "apps", "desktop", "dist", "assets");
  const assets = readdirSync(assetsDir)
    .filter((name) => name.endsWith(".js"))
    .map((name) => ({ bytes: statSync(join(assetsDir, name)).size, name }));

  const { failures, report } = checkBundleBudgets(assets);
  for (const line of report) console.log(`✓ ${line}`);

  if (failures.length > 0) {
    console.error("\nFrontend bundle budgets failed:\n");
    for (const failure of failures) console.error(`- ${failure}`);
    console.error("\nBudgets live in scripts/quality/frontend-bundle.mjs.");
    process.exit(1);
  }
}
