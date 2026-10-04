import { describe, expect, it } from "vitest";

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
