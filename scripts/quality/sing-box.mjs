import { join } from "node:path";
import { stripVTControlCharacters } from "node:util";

import { capture, describeCommand, isCliEntrypoint, repoRootFromScript } from "../lib/common.mjs";
import {
  ensureSingBoxSeedForBuild,
  singBoxExecutableName,
} from "../core/sing-box-installer.mjs";

/**
 * The one voya-core test that feeds every acceptance config to `sing-box check`.
 * `golden` is declared `#[cfg(test)] mod golden;` in crates/voya-core/src/lib.rs,
 * and sing-box.test.mjs fails if either half of this path stops existing.
 */
export const ACCEPTANCE_TEST = "golden::golden_core_acceptance_checks_are_opt_in";

/**
 * Only the lib target, and `--exact`, so the filter names one test: a cargo
 * test filter that matches nothing still exits 0, which is how a renamed test
 * would otherwise turn this gate silently green.
 */
export const ACCEPTANCE_TEST_ARGS = [
  "test",
  "-p",
  "voya-core",
  "--lib",
  "--",
  "--exact",
  ACCEPTANCE_TEST,
  "--nocapture",
];

const LIBTEST_SUMMARY =
  /^test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/gmu;

/**
 * Returns why a libtest stdout does not prove that `testName` ran and passed as
 * the only test, or `null` when it does.
 */
export function acceptanceRunProblem(stdout, testName = ACCEPTANCE_TEST) {
  // libtest colours `ok`/`FAILED` when told to, which would split the summary.
  const text = stripVTControlCharacters(String(stdout ?? ""));
  const summaries = [...text.matchAll(LIBTEST_SUMMARY)].map((match) => ({
    outcome: match[1],
    passed: Number(match[2]),
    failed: Number(match[3]),
    ignored: Number(match[4]),
  }));

  if (summaries.length !== 1) {
    return `expected one libtest summary for the voya-core lib target, found ${summaries.length}`;
  }
  const [summary] = summaries;
  if (summary.outcome !== "ok" || summary.passed !== 1 || summary.failed !== 0 || summary.ignored !== 0) {
    return `expected ${testName} to be the one passing test, got ${summary.passed} passed, ${summary.failed} failed, ${summary.ignored} ignored`;
  }
  // `--nocapture` prints the test's own output between the name and `ok`, so
  // only the start of the line is stable.
  if (!text.split(/\r?\n/u).some((line) => line.startsWith(`test ${testName} ... `))) {
    return `${testName} did not run`;
  }
  return null;
}

if (isCliEntrypoint(import.meta.url)) {
  const repoRoot = repoRootFromScript(import.meta.url);
  const { seedDir } = await ensureSingBoxSeedForBuild({ repoRoot });
  const executable = join(seedDir, singBoxExecutableName());

  // stdout is captured for the libtest summary and echoed afterwards; cargo's
  // build progress on stderr still streams live.
  const result = capture("cargo", ACCEPTANCE_TEST_ARGS, {
    env: {
      ...process.env,
      VOYA_GOLDEN_ACCEPTANCE: "1",
      VOYA_SINGBOX_BIN: executable,
    },
    maxBuffer: 64 * 1024 * 1024,
    stdio: ["inherit", "pipe", "inherit"],
  });
  process.stdout.write(result.stdout ?? "");

  // The captured stdout above is the only record of what cargo said, and an
  // uncaught throw tears the process down without draining a piped stdout —
  // truncating exactly the run that failed. Set the exit code and return
  // instead, so node flushes on its own.
  const problem = result.status === 0 && !result.error ? acceptanceRunProblem(result.stdout) : null;
  const failure = result.error
    ? String(result.error.message ?? result.error)
    : result.status !== 0
      ? `${describeCommand("cargo", ACCEPTANCE_TEST_ARGS)} failed with status ${result.status}`
      : problem
        ? `sing-box config acceptance did not run: ${problem}`
        : null;

  if (failure) {
    console.error(`\n${failure}`);
    process.exitCode = 1;
  }
}
