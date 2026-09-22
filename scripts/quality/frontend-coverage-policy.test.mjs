import { describe, expect, it } from "vitest";

import { checkBundleBudgets, checkDistBudgets, checkStartupBudget, startupScripts } from "./frontend-bundle.mjs";
import {
  criticalModules,
  evaluateCoverage,
  globalMinimums,
  runtimeModules,
} from "./frontend-coverage-policy.mjs";

function metrics(lines, branches = lines, functions = lines, statements = lines) {
  return {
    lines: { pct: lines },
    branches: { pct: branches },
    functions: { pct: functions },
    statements: { pct: statements },
  };
}

/** A summary in which every listed module is comfortably green. */
function healthySummary(overrides = {}) {
  const entries = new Map();
  for (const path of criticalModules) entries.set(path, metrics(100));
  for (const module of runtimeModules) entries.set(module.path, metrics(100));
  for (const [path, value] of Object.entries(overrides)) entries.set(path, value);
  return entries;
}

function evaluate(entries, total = metrics(90, 85, 90, 90), policy = {}) {
  return evaluateCoverage({ total, lookup: (path) => entries.get(path), policy });
}

describe("frontend coverage policy", () => {
  it("passes when the totals and every listed module clear their floors", () => {
    expect(evaluate(healthySummary()).failures).toEqual([]);
  });

  it("fails when a global metric drops below its minimum", () => {
    const { failures } = evaluate(healthySummary(), metrics(90, globalMinimums.branches - 0.5, 90, 90));

    expect(failures).toEqual([`total: branches ${globalMinimums.branches - 0.5}% < ${globalMinimums.branches}%`]);
  });

  // The old gate listed only files already at ~100%, so a real regression in a
  // runtime module was absorbed by the global average.
  it("fails a runtime module that regresses below its ratchet floor", () => {
    const module = runtimeModules.find((entry) => entry.path === "apps/desktop/src/ipc/event-bridge.tsx");
    const { failures } = evaluate(
      healthySummary({ [module.path]: metrics(100, module.branches - 1, 100, 100) }),
    );

    expect(failures).toEqual([
      `${module.path}: branches ${module.branches - 1}% < ${module.branches}% (runtime floor)`,
    ]);
  });

  it("fails a critical module that drops under 80% on any single metric", () => {
    const path = criticalModules[0];
    const { failures } = evaluate(healthySummary({ [path]: metrics(100, 79.9, 100, 100) }));

    expect(failures).toEqual([`${path}: branches 79.9% < 80%`]);
  });

  it("fails when a listed module disappears from the report", () => {
    const entries = healthySummary();
    entries.delete(criticalModules[0]);
    entries.delete(runtimeModules[0].path);

    expect(evaluate(entries).failures).toEqual([
      `${criticalModules[0]}: missing from the coverage report`,
      `${runtimeModules[0].path}: missing from the coverage report`,
    ]);
  });

  it("keeps the runtime tier disjoint from the critical tier", () => {
    const runtimePaths = new Set(runtimeModules.map((module) => module.path));

    for (const path of criticalModules) {
      expect(runtimePaths.has(path), path).toBe(false);
    }
  });
});

describe("frontend bundle budgets", () => {
  const assets = [
    { name: "index-abc.js", bytes: 43 * 1024 },
    { name: "locales-abc.js", bytes: 40 * 1024 },
    { name: "zh-Hans-abc.js", bytes: 39 * 1024 },
    { name: "zh-Hant-abc.js", bytes: 40 * 1024 },
    { name: "server-table-abc.js", bytes: 130 * 1024 },
    { name: "settings-screen-abc.js", bytes: 64 * 1024 },
    { name: "vendor-data-abc.js", bytes: 79 * 1024 },
    { name: "vendor-react-abc.js", bytes: 186 * 1024 },
    { name: "vendor-radix-abc.js", bytes: 59 * 1024 },
    { name: "vendor-forms-abc.js", bytes: 97 * 1024 },
    { name: "vendor-menus-abc.js", bytes: 97 * 1024 },
  ];

  it("accepts the sizes the budgets were ratcheted around", () => {
    expect(checkBundleBudgets(assets).failures).toEqual([]);
  });

  it("reports every breach rather than throwing on the first", () => {
    const bloated = assets.map((asset) => ({ ...asset, bytes: asset.bytes * 4 }));
    const { failures } = checkBundleBudgets(bloated);

    expect(failures.length).toBe(assets.length + 1);
    expect(failures.at(-1)).toContain("total emitted JavaScript");
  });

  it("fails when a budgeted chunk is not emitted at all", () => {
    const { failures } = checkBundleBudgets(assets.filter((asset) => !asset.name.startsWith("index-")));

    expect(failures).toEqual(["application entry bundle (index-*.js) was not generated"]);
  });

  it("catches a total-size regression that no single chunk budget would catch", () => {
    const many = [...assets, ...Array.from({ length: 20 }, (_, index) => ({ name: `chunk-${index}.js`, bytes: 40 * 1024 }))];
    const { failures } = checkBundleBudgets(many);

    expect(failures).toHaveLength(1);
    expect(failures[0]).toContain("total emitted JavaScript");
  });
});

describe("frontend startup budget", () => {
  const html = `<!doctype html>
    <script src="/theme-boot.js"></script>
    <script type="module" crossorigin src="/assets/index-abc.js"></script>
    <link rel="modulepreload" crossorigin href="/assets/vendor-react-abc.js">
    <link rel="modulepreload" crossorigin href="/assets/vendor-data-abc.js">
    <link rel="stylesheet" crossorigin href="/assets/index-abc.css">`;
  const sizes = { "assets/index-abc.js": 48, "assets/vendor-react-abc.js": 186, "assets/vendor-data-abc.js": 79 };
  const sizeOf = (script) => sizes[script] * 1024;

  it("counts the entry and its preloads, not the stylesheet or public scripts", () => {
    expect(startupScripts(html)).toEqual(["assets/index-abc.js", "assets/vendor-react-abc.js", "assets/vendor-data-abc.js"]);
    expect(checkStartupBudget(startupScripts(html), sizeOf).failures).toEqual([]);
  });

  it("catches a lazy library pulled onto the startup path", () => {
    const withForms = `${html}<link rel="modulepreload" crossorigin href="/assets/vendor-forms-abc.js">`;
    const bigger = (script) => (script.includes("vendor-forms") ? 300 * 1024 : sizeOf(script));

    expect(checkStartupBudget(startupScripts(withForms), bigger).failures).toEqual([
      expect.stringContaining("startup JavaScript"),
    ]);
  });

  it("fails when index.html loads nothing it could measure", () => {
    expect(checkStartupBudget([], sizeOf).failures).toEqual(["index.html loads no script from assets/"]);
  });
});

describe("frontend dist budgets", () => {
  const files = [
    { name: "index.html", bytes: 2 * 1024 },
    { name: "index-abc.css", bytes: 80 * 1024 },
    { name: "index-abc.js", bytes: 1500 * 1024 },
    { name: "us-abc.svg", bytes: 1700 * 1024 },
  ];

  it("accepts the sizes the budgets were ratcheted around", () => {
    expect(checkDistBudgets(files).failures).toEqual([]);
  });

  it("catches a stylesheet that inlines assets and assets JavaScript budgets never see", () => {
    const flags = [
      ...files,
      { name: "vendor-abc.css", bytes: 421 * 1024 },
      ...Array.from({ length: 142 }, (_, index) => ({ name: `flag-${index}.svg`, bytes: 26 * 1024 })),
    ];

    expect(checkDistBudgets(flags).failures).toEqual([
      expect.stringContaining("total CSS"),
      expect.stringContaining("whole embedded dist"),
    ]);
  });
});
