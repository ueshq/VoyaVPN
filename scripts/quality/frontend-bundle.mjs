import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { isCliEntrypoint, repoRootFromScript } from "../lib/common.mjs";

/**
 * Bundle budgets are a ratchet, not a ceiling nobody can reach: each budget
 * sits roughly 1.3-1.5x the size the chunk had when it was set, so an
 * accidental dependency import is caught while ordinary feature work is not.
 * **Lower the budget when a chunk shrinks.** Sizes recorded 2026-09-08:
 * index 54.5 KiB, locales 249.8 KiB, vendor-qr 456.8 KiB, vendor-data
 * 241.0 KiB, vendor-react 185.2 KiB, vendor-radix 147.6 KiB, server-table
 * 70.9 KiB, settings-screen 40.8 KiB; total emitted JS 1618 KiB.
 *
 * The entry budget dropped from 300 KiB when the locale JSON files moved into
 * their own chunk (`locales`, see `vite.config.ts`): they were four fifths of
 * the entry, so the old budget had stopped measuring application code at all.
 * Translation growth is now ratcheted on its own line.
 *
 * JavaScript alone missed the largest thing Tauri embeds: the flag-icons
 * stylesheet put every flag into the bundle (542 SVGs, ~4 MB, 421 KiB of it
 * inlined into a startup CSS file). CSS and the whole `dist` are budgeted too.
 * Sizes recorded 2026-09-18, after flags were cut to the 4x3 two-letter set:
 * CSS 78.5 KiB, whole `dist` 3328 KiB (1717 KiB of it flag SVGs).
 *
 * 2026-09-19: React Compiler was enabled (`vite.config.ts`). Its memoization
 * code grows component chunks by roughly a third to two thirds, which was
 * accepted for fewer re-renders on screens fed per frame; the profiles screen
 * budget moved up with it. The locales now load on demand except English, so
 * `locales` is the English fallback alone and each other locale has a line.
 * Sizes recorded then: index 43.1 KiB, locales 40.0 KiB, zh-Hans 39.3 KiB,
 * zh-Hant 39.8 KiB, server-table 129.4 KiB, settings-screen 64.2 KiB; total
 * emitted JS 1677 KiB; whole `dist` 3475 KiB.
 *
 * 2026-09-19, later: the vendor groups only capture modules the entry loads
 * (`startupGroup` in `vite.config.ts`). The form library and the menus' Radix
 * packages had been riding in startup chunks; they are `vendor-forms` and
 * `vendor-menus` now, loaded with the first screen that uses them, and the
 * startup path has its own budget. Sizes recorded then: vendor-data 78.6 KiB,
 * vendor-radix 59.0 KiB, vendor-forms 96.9 KiB, vendor-menus 96.3 KiB;
 * startup JavaScript 501.3 KiB (724.8 KiB before).
 */
const budgets = [
  { label: "application entry", maxKiB: 60, prefix: "index-" },
  { label: "English locale (startup)", maxKiB: 55, prefix: "locales-" },
  { label: "Simplified Chinese locale", maxKiB: 55, prefix: "zh-Hans-" },
  { label: "Traditional Chinese locale", maxKiB: 55, prefix: "zh-Hant-" },
  { label: "profiles screen", maxKiB: 170, prefix: "server-table-" },
  { label: "settings screen", maxKiB: 85, prefix: "settings-screen-" },
  { label: "QR decoder", maxKiB: 500, prefix: "vendor-qr-" },
  { label: "data vendor chunk", maxKiB: 110, prefix: "vendor-data-" },
  { label: "React vendor chunk", maxKiB: 240, prefix: "vendor-react-" },
  { label: "Radix vendor chunk", maxKiB: 85, prefix: "vendor-radix-" },
  { label: "form vendor chunk", maxKiB: 135, prefix: "vendor-forms-" },
  { label: "menu vendor chunk", maxKiB: 135, prefix: "vendor-menus-" },
];

const totalBudgetKiB = 1900;
/**
 * Every script `index.html` loads before the first paint. Tighter than the
 * per-chunk ratchet on purpose: an accidental startup import is exactly what
 * this line exists to catch, and 1.3x of the whole path would let a form
 * library through unnoticed.
 */
const startupBudgetKiB = 575;
const cssBudgetKiB = 120;
const distBudgetKiB = 4000;

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

/**
 * Budgets for what the JavaScript chunks do not cover: every stylesheet
 * together, and every file Tauri embeds from `dist`.
 */
export function checkDistBudgets(files, { cssKiB = cssBudgetKiB, distKiB = distBudgetKiB } = {}) {
  const failures = [];
  const report = [];
  const kib = (entries) => entries.reduce((sum, file) => sum + file.bytes, 0) / 1024;

  for (const [label, actualKiB, maxKiB] of [
    ["total CSS", kib(files.filter((file) => file.name.endsWith(".css"))), cssKiB],
    ["whole embedded dist", kib(files), distKiB],
  ]) {
    if (actualKiB > maxKiB) {
      failures.push(`${label} is ${actualKiB.toFixed(1)} KiB; budget is ${maxKiB} KiB`);
    } else {
      report.push(`${label}: ${actualKiB.toFixed(1)} KiB / ${maxKiB} KiB`);
    }
  }

  return { failures, report };
}

/** The scripts `html` loads before the first paint: its entry and preloads. */
export function startupScripts(html) {
  return [...html.matchAll(/<(?:script|link)\b[^>]*\b(?:src|href)="\/?(assets\/[^"]+\.js)"/g)].map((match) => match[1]);
}

export function checkStartupBudget(scripts, sizeOf, { startupKiB = startupBudgetKiB } = {}) {
  const actualKiB = scripts.reduce((sum, script) => sum + sizeOf(script), 0) / 1024;
  if (scripts.length === 0) {
    return { failures: ["index.html loads no script from assets/"], report: [] };
  }
  if (actualKiB > startupKiB) {
    return {
      failures: [`startup JavaScript (${scripts.length} files) is ${actualKiB.toFixed(1)} KiB; budget is ${startupKiB} KiB`],
      report: [],
    };
  }
  return { failures: [], report: [`startup JavaScript: ${actualKiB.toFixed(1)} KiB / ${startupKiB} KiB`] };
}

function listFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    return entry.isDirectory() ? listFiles(path) : [{ bytes: statSync(path).size, name: entry.name }];
  });
}

if (isCliEntrypoint(import.meta.url)) {
  const distDir = join(repoRootFromScript(import.meta.url), "apps", "desktop", "dist");
  const assetsDir = join(distDir, "assets");
  const assets = readdirSync(assetsDir)
    .filter((name) => name.endsWith(".js"))
    .map((name) => ({ bytes: statSync(join(assetsDir, name)).size, name }));

  const js = checkBundleBudgets(assets);
  const startup = checkStartupBudget(
    startupScripts(readFileSync(join(distDir, "index.html"), "utf8")),
    (script) => statSync(join(distDir, script)).size,
  );
  const dist = checkDistBudgets(listFiles(distDir));
  const failures = [...js.failures, ...startup.failures, ...dist.failures];
  const report = [...js.report, ...startup.report, ...dist.report];
  for (const line of report) console.log(`✓ ${line}`);

  if (failures.length > 0) {
    console.error("\nFrontend bundle budgets failed:\n");
    for (const failure of failures) console.error(`- ${failure}`);
    console.error("\nBudgets live in scripts/quality/frontend-bundle.mjs.");
    process.exit(1);
  }
}
