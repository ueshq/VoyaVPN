import { describe, expect, it } from "vitest";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { commandFailure, describeCommand, environmentValue, redactArgs, run, validateTiming } from "./common.mjs";

describe("shared native tool helpers", () => {
  it("reports status and stderr while redacting command arguments", () => {
    expect(commandFailure("tool", ["--token", "secret"], { status: 7, stdout: "out", stderr: "err" }).message)
      .toBe("tool --token *** failed with status 7: err");
    expect(commandFailure("tool", [], { status: null, stdout: " out " }).message)
      .toBe("tool failed with status unknown: out");
  });

  it("reads Windows environment names without changing the first non-empty match", () => {
    expect(environmentValue({ programfiles: "  ", PROGRAMW6432: " C:\\Apps " }, "ProgramFiles", "ProgramW6432")).toBe("C:\\Apps");
    expect(environmentValue({ windir: "first", SystemRoot: "second" }, "SystemRoot", "windir")).toBe("first");
    expect(environmentValue(undefined, "SystemRoot")).toBe("");
  });

  it("allows an immediate deadline and rejects invalid polling intervals", () => {
    expect(() => validateTiming(0, 1)).not.toThrow();
    for (const timeout of [-1, Infinity, NaN]) expect(() => validateTiming(timeout, 1)).toThrow("timeoutMs");
    for (const interval of [0, -1, Infinity, NaN]) expect(() => validateTiming(1, interval)).toThrow("pollIntervalMs");
  });
});

describe("command argument redaction", () => {
  it("masks credential values that follow a secret flag", () => {
    expect(
      redactArgs([
        "notarytool",
        "submit",
        "VoyaVPN.zip",
        "--apple-id",
        "releases@voyavpn.dev",
        "--team-id",
        "TEAM123",
        "--password",
        "abcd-efgh-ijkl-mnop",
      ]),
    ).toEqual([
      "notarytool",
      "submit",
      "VoyaVPN.zip",
      "--apple-id",
      "***",
      "--team-id",
      "TEAM123",
      "--password",
      "***",
    ]);
  });

  it("masks inline credential values but leaves short flags alone", () => {
    expect(redactArgs(["--password=abcd-efgh", "--token=xyz", "-p", "voya-core"])).toEqual([
      "--password=***",
      "--token=***",
      "-p",
      "voya-core",
    ]);
  });

  it("leaves ordinary arguments untouched", () => {
    const args = ["--force", "--sign", "1A2B3C", "--timestamp", "target/release/VoyaVPN.dmg"];

    expect(redactArgs(args)).toEqual(args);
    expect(describeCommand("codesign", args)).toBe(`codesign ${args.join(" ")}`);
  });

  it("keeps secrets out of the failure message thrown by run()", () => {
    const scriptDir = mkdtempSync(join(tmpdir(), "voyavpn-common-"));
    const script = join(scriptDir, "fail.mjs");
    writeFileSync(script, "process.exit(3);\n");

    expect(() => run(process.execPath, [script, "--password", "hunter2"])).toThrow(/--password \*\*\*/);
    expect(() => run(process.execPath, [script, "--password", "hunter2"])).not.toThrow(/hunter2/);
  });
});
