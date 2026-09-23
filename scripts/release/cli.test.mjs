import { describe, expect, it, vi } from "vitest";

import { capture, repoRootFromScript } from "../lib/common.mjs";
import { releaseCommandNames, runReleaseCli } from "./cli.mjs";

const repoRoot = repoRootFromScript(import.meta.url);

function stream() {
  let value = "";
  return {
    stream: { write: (chunk) => { value += String(chunk); } },
    value: () => value,
  };
}

describe("release CLI", () => {
  it("lists every supported command", async () => {
    const stdout = stream();
    const exitCode = await runReleaseCli(["--help"], { stdout: stdout.stream });

    expect(exitCode).toBe(0);
    for (const command of releaseCommandNames) expect(stdout.value()).toContain(command);
  });

  it("accepts pnpm's explicit argument separator", async () => {
    const stdout = stream();
    const exitCode = await runReleaseCli(["--", "--help"], { stdout: stdout.stream });

    expect(exitCode).toBe(0);
    expect(stdout.value()).toContain("Commands:");
  });

  it("uses exit code 2 for an unknown command", async () => {
    const stderr = stream();
    const exitCode = await runReleaseCli(["missing"], { stderr: stderr.stream });

    expect(exitCode).toBe(2);
    expect(stderr.value()).toContain("Unknown release command: missing");
  });

  it.each(releaseCommandNames)("serves integration help for %s", (command) => {
    const result = capture(process.execPath, ["scripts/release/cli.mjs", command, "--help"], {
      cwd: repoRoot,
    });

    expect(result.status, result.stderr).toBe(0);
    expect(result.stdout).toContain("Usage:");
  });

  it("returns integration exit code 2 for an unknown command", () => {
    const result = capture(process.execPath, ["scripts/release/cli.mjs", "missing"], {
      cwd: repoRoot,
    });

    expect(result.status).toBe(2);
    expect(result.stderr).toContain("Unknown release command: missing");
  });

  it("dispatches command arguments without executing modules on import", async () => {
    const main = vi.fn();
    const exitCode = await runReleaseCli(["index", "--help"], {
      loaders: { index: async () => ({ main }) },
    });

    expect(exitCode).toBe(0);
    expect(main).toHaveBeenCalledWith(["--help"]);
  });
});
