import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { repoRootFromScript } from "../lib/common.mjs";
import { steps } from "./verify-local.mjs";

const repoRoot = repoRootFromScript(import.meta.url);

function read(relativePath) {
  return readFileSync(resolve(repoRoot, relativePath), "utf8");
}

const gates = steps.map(([, , args]) => args[1]);
const packageScripts = JSON.parse(read("package.json")).scripts;

/**
 * The `pnpm run check:*` gates each top-level ci.yml job runs, keyed by job id.
 * Comment lines are skipped so a comment that names a gate cannot count as
 * running it.
 */
function ciGatesByJob() {
  const lines = read(".github/workflows/ci.yml").split(/\r?\n/u);
  const jobs = new Map();
  let current = null;
  for (const line of lines.slice(lines.indexOf("jobs:") + 1)) {
    const header = /^ {2}([A-Za-z0-9_-]+):\s*$/u.exec(line);
    if (header) {
      current = header[1];
      jobs.set(current, []);
    } else if (current && !line.trim().startsWith("#")) {
      jobs.get(current).push(...[...line.matchAll(/pnpm run (check:[\w:-]+)/gu)].map((match) => match[1]));
    }
  }
  return jobs;
}

const baselineJobs = [...ciGatesByJob()].filter(([job]) => job.startsWith("baseline"));

describe("verify:local is the single source of truth for the gate list", () => {
  it("runs only scripts that package.json actually defines", () => {
    for (const gate of gates) expect(Object.keys(packageScripts), gate).toContain(gate);
  });

  // The CI baseline and verify:local drifted before: AGENTS.md advertised
  // `check:frontend:test` while CI ran `check:frontend:coverage`, and neither
  // list mentioned `check:architecture` at all. CI now splits the gates across
  // parallel baseline-* jobs, so order is free but the set is not: every gate
  // runs in exactly one of them, and none of them runs anything else.
  it("is covered gate for gate by the CI baseline-* jobs", () => {
    expect(baselineJobs.map(([job]) => job)).not.toEqual([]);
    const inCi = baselineJobs.flatMap(([, jobGates]) => jobGates);
    const repeated = inCi.filter((gate, index) => inCi.indexOf(gate) !== index);

    expect(repeated).toEqual([]);
    expect([...inCi].sort()).toEqual([...gates].sort());
  });

  it("runs each canonical gate exactly once", () => {
    expect(new Set(gates).size).toBe(gates.length);
  });

  it.each(["AGENTS.md", "README.md"])("is documented gate for gate in %s", (document) => {
    const text = read(document);
    const missing = gates.filter((gate) => !text.includes(gate));

    expect(missing).toEqual([]);
  });

  it("does not let the docs advertise a gate that no longer exists", () => {
    for (const document of ["AGENTS.md", "README.md"]) {
      const documented = [...read(document).matchAll(/pnpm (?:run )?(check:[\w:-]+)/gu)].map((match) => match[1]);
      const unknown = [...new Set(documented)].filter((gate) => !Object.keys(packageScripts).includes(gate));

      expect(unknown, document).toEqual([]);
    }
  });
});
