import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

import { commands, resolveCommand } from "../commands.mjs";
import { repoRootFromScript } from "../lib/common.mjs";
import { steps } from "./verify-local.mjs";

const repoRoot = repoRootFromScript(import.meta.url);

function read(relativePath) {
  return readFileSync(resolve(repoRoot, relativePath), "utf8");
}

/** A gate as it is typed: `vp check`, or the words after `vp run`. */
const gates = steps.map(([, command, args]) => (args[0] === "run" ? args.slice(1) : [command, ...args]).join(" "));
const builtinGate = "vp check";

/**
 * A command invocation, `vp run check x y`. Workflows never call pnpm
 * directly (`workflows.test.mjs` rejects it), so this is the one spelling.
 * The words run on into whatever follows in prose, so the command table
 * decides where the name ends; words it does not know are kept whole, to be
 * reported as a gate that does not exist. The built-in `vp check` is a gate
 * too, but only bare: `vp check --fix` rewrites files, it gates nothing.
 */
const gateInvocation = /(?<![\w-])vp (?:run (check(?: [\w-]+)+)|check(?![\w:-]| +[\w-]))/gu;

/** The gates one line runs. */
function gatesInLine(line) {
  return [...line.matchAll(gateInvocation)].map((match) =>
    match[1] ? (resolveCommand(match[1].split(" "))?.name ?? match[1]) : builtinGate,
  );
}

/**
 * The `check …` gates each top-level ci.yml job runs, keyed by job id.
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
    ["        run: vp run check architecture", ["check architecture"]],
    ["        run: vp run check rust fmt && vp run check rust clippy", ["check rust fmt", "check rust clippy"]],
    ["        run: xvfb-run -a vp run check desktop smoke", ["check desktop smoke"]],
    ["run `vp run check i18n` and read the output", ["check i18n"]],
    ["        run: vp run check retired gate", ["check retired gate"]],
    ["        run: vp run --filter @voya/web build", []],
    ["        run: vp check", ["vp check"]],
    ["        run: vp check --fix", []],
    ["        run: vp check architecture", []],
    ["        run: vp check:architecture", []],
    ["        run: xvp run check architecture", []],
  ])("reads the gates of %j", (line, expected) => {
    expect(gatesInLine(line)).toEqual(expected);
  });
});

describe("verify local is the single source of truth for the gate list", () => {
  it("runs only commands that scripts/commands.mjs actually defines", () => {
    for (const gate of gates) expect([builtinGate, ...Object.keys(commands)], gate).toContain(gate);
  });

  // The CI baseline and verify local drifted before: AGENTS.md advertised
  // a test-only gate while CI ran `check frontend coverage`, and neither
  // list mentioned `check architecture` at all. CI now splits the gates across
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
      const unknown = [...new Set(gatesInLine(read(document)))].filter(
        (gate) => gate !== builtinGate && !Object.hasOwn(commands, gate),
      );

      expect(unknown, document).toEqual([]);
    }
  });
});
