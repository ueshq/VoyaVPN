import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { repoRootFromScript } from "../lib/common.mjs";
import { ACCEPTANCE_TEST, ACCEPTANCE_TEST_ARGS, acceptanceRunProblem } from "./sing-box.mjs";

const repoRoot = repoRootFromScript(import.meta.url);

function libtestOutput({ running, lines = [], passed, failed = 0, ignored = 0, filtered }) {
  return [
    "",
    `running ${running} test${running === 1 ? "" : "s"}`,
    ...lines,
    "",
    `test result: ${failed === 0 ? "ok" : "FAILED"}. ${passed} passed; ${failed} failed; ${ignored} ignored; 0 measured; ${filtered} filtered out; finished in 1.23s`,
    "",
  ].join("\n");
}

describe("sing-box acceptance run verification", () => {
  it("accepts a run where exactly the acceptance test passed", () => {
    const stdout = libtestOutput({
      running: 1,
      lines: [
        `test ${ACCEPTANCE_TEST} ... sing-box check passed for vless-ws-tls-mux-0`,
        "ok",
      ],
      passed: 1,
      filtered: 212,
    });

    expect(acceptanceRunProblem(stdout)).toBeNull();
  });

  it("reads coloured libtest output", () => {
    const green = (text) => `\u001b[32m${text}\u001b[0m`;
    const stdout = [
      "running 1 test",
      `test ${ACCEPTANCE_TEST} ... ${green("ok")}`,
      "",
      `test result: ${green("ok")}. 1 passed; 0 failed; 0 ignored; 0 measured; 212 filtered out; finished in 1.23s`,
    ].join("\n");

    expect(acceptanceRunProblem(stdout)).toBeNull();
  });

  it("rejects a filter that matched no test, which cargo still exits 0 for", () => {
    const stdout = libtestOutput({ running: 0, passed: 0, filtered: 213 });

    expect(acceptanceRunProblem(stdout)).toContain("0 passed");
  });

  it("rejects a run where the acceptance test was filtered out behind another test", () => {
    const stdout = libtestOutput({
      running: 1,
      lines: ["test golden::golden_matrix_matches ... ok"],
      passed: 1,
      filtered: 212,
    });

    expect(acceptanceRunProblem(stdout)).toContain("did not run");
  });

  it("rejects a filter that over-matched", () => {
    const stdout = libtestOutput({
      running: 2,
      lines: [`test ${ACCEPTANCE_TEST} ... ok`, `test ${ACCEPTANCE_TEST}_v2 ... ok`],
      passed: 2,
      filtered: 211,
    });

    expect(acceptanceRunProblem(stdout)).toContain("2 passed");
  });

  it("rejects failed or ignored runs", () => {
    expect(
      acceptanceRunProblem(
        libtestOutput({ running: 1, lines: [`test ${ACCEPTANCE_TEST} ... FAILED`], passed: 0, failed: 1, filtered: 212 }),
      ),
    ).toContain("1 failed");
    expect(
      acceptanceRunProblem(
        libtestOutput({ running: 1, lines: [`test ${ACCEPTANCE_TEST} ... ignored`], passed: 0, ignored: 1, filtered: 212 }),
      ),
    ).toContain("1 ignored");
  });

  it("rejects output from more than one test target", () => {
    const lib = libtestOutput({ running: 1, lines: [`test ${ACCEPTANCE_TEST} ... ok`], passed: 1, filtered: 212 });
    const doctests = libtestOutput({ running: 0, passed: 0, filtered: 4 });

    expect(acceptanceRunProblem(`${lib}${doctests}`)).toContain("found 2");
    expect(acceptanceRunProblem("")).toContain("found 0");
  });

  it("names the test exactly and runs only the voya-core lib target", () => {
    expect(ACCEPTANCE_TEST_ARGS).toEqual(expect.arrayContaining(["-p", "voya-core", "--lib", "--exact", ACCEPTANCE_TEST]));
    expect(ACCEPTANCE_TEST_ARGS.indexOf("--exact")).toBeGreaterThan(ACCEPTANCE_TEST_ARGS.indexOf("--"));
  });

  // A rename in Rust has to break this test rather than the gate.
  it("points at a test that exists in crates/voya-core", () => {
    const [module, name] = ACCEPTANCE_TEST.split("::");
    const lib = readFileSync(resolve(repoRoot, "crates/voya-core/src/lib.rs"), "utf8");
    const golden = readFileSync(resolve(repoRoot, `crates/voya-core/src/${module}.rs`), "utf8");

    expect(lib).toMatch(new RegExp(`^\\s*(?:pub(?:\\([^)]*\\))?\\s+)?mod\\s+${module}\\s*;`, "mu"));
    expect(golden).toMatch(new RegExp(`#\\[test\\]\\s*fn\\s+${name}\\s*\\(`, "u"));
  });
});
