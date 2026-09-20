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
 * The `check:*` gates one workflow line runs. pnpm runs a package script with
 * or without `run`, so both spellings count; otherwise a step written
 * `pnpm check:x` would be invisible to the parity test below.
 */
function gatesInLine(line) {
  return [...line.matchAll(/\bpnpm (?:run )?(check:[\w:-]+)/gu)].map((match) => match[1]);
}

/**
 * The `check:*` gates each top-level ci.yml job runs, keyed by job id.
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
      jobs.get(current).push(...gatesInLine(line));
    }
  }
  return jobs;
}

const baselineJobs = [...ciGatesByJob()].filter(([job]) => job.startsWith("baseline"));

describe("CI gate discovery", () => {
  it.each([
    ["        run: pnpm run check:architecture", ["check:architecture"]],
    ["        run: pnpm check:architecture", ["check:architecture"]],
    ["        run: pnpm run check:rust:fmt && pnpm check:rust:clippy", ["check:rust:fmt", "check:rust:clippy"]],
    ["        run: pnpm --filter @voya/desktop build", []],
    ["        run: xpnpm check:architecture", []],
  ])("reads the gates of %j", (line, expected) => {
    expect(gatesInLine(line)).toEqual(expected);
  });
});

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
