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
const ci = read(".github/workflows/ci.yml");
const baseline = ci.slice(ci.indexOf("\n  baseline:"), ci.indexOf("\n  desktop-smoke:"));

describe("verify:local is the single source of truth for the gate list", () => {
  it("runs only scripts that package.json actually defines", () => {
    for (const gate of gates) expect(Object.keys(packageScripts), gate).toContain(gate);
  });

  // The CI baseline job and verify:local drifted before: AGENTS.md advertised
  // `check:frontend:test` while CI ran `check:frontend:coverage`, and neither
  // list mentioned `check:architecture` at all.
  it("is mirrored step for step by the CI baseline job", () => {
    expect(baseline).not.toBe("");
    const missing = gates.filter((gate) => !baseline.includes(`pnpm run ${gate}`));

    expect(missing).toEqual([]);
  });

  it("leaves no baseline gate out of verify:local", () => {
    const inCi = [...baseline.matchAll(/pnpm run (check:[\w:-]+)/gu)].map((match) => match[1]);
    const extra = [...new Set(inCi)].filter((gate) => !gates.includes(gate));

    expect(extra).toEqual([]);
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
