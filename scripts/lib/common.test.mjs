import { afterEach, describe, expect, it } from "vitest";
import { mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { checkedCapture, commandFailure, describeCommand, environmentValue, run, validateTiming } from "./common.mjs";

const temporaryDirectories = [];
afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { recursive: true, force: true });
  }
});

function commandFixture(code) {
  const directory = realpathSync(mkdtempSync(join(tmpdir(), "voyavpn-common-")));
  temporaryDirectories.push(directory);
  const script = join(directory, "command.mjs");
  writeFileSync(script, code);
  return { directory, script };
}

describe("checked command capture", () => {
  it("preserves output without trimming and returns the process result", () => {
    const { script } = commandFixture('process.stdout.write(" out\\n"); process.stderr.write("err\\n");');
    expect(checkedCapture(process.execPath, [script])).toMatchObject({
      status: 0, stdout: " out\n", stderr: "err\n",
    });
  });

  it("throws the spawn error when a command cannot be started", () => {
    const { directory } = commandFixture("");
    expect(() => checkedCapture(join(directory, "missing-command"), [])).toThrow(
      expect.objectContaining({ code: "ENOENT" }),
    );
  });

  it.each(["stderr", "stdout"])("reports failed command %s and redacts credentials", (stream) => {
    const { script } = commandFixture(`process.${stream}.write("failure detail\\n"); process.exit(7);`);
    let failure;
    try {
      checkedCapture(process.execPath, [script, "--password", "secret-value", "--token=secret-token"]);
    } catch (error) {
      failure = error;
    }
    expect(failure).toBeInstanceOf(Error);
    expect(failure.message).toContain("failed with status 7: failure detail");
    expect(failure.message).toContain("--password *** --token=***");
    expect(failure.message).not.toContain("secret-value");
    expect(failure.message).not.toContain("secret-token");
  });
});

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
      describeCommand("notarytool", [
        "submit",
        "VoyaVPN.zip",
        "--apple-id",
        "releases@voyavpn.dev",
        "--team-id",
        "TEAM123",
        "--password",
        "abcd-efgh-ijkl-mnop",
      ]),
    ).toBe("notarytool submit VoyaVPN.zip --apple-id *** --team-id TEAM123 --password ***");
  });

  it("masks inline credential values but leaves short flags alone", () => {
    expect(describeCommand("cargo", ["--password=abcd-efgh", "--token=xyz", "-p", "voya-core"])).toBe(
      "cargo --password=*** --token=*** -p voya-core",
    );
  });

  it("leaves ordinary arguments untouched", () => {
    const args = ["--force", "--sign", "1A2B3C", "--timestamp", "target/release/VoyaVPN.dmg"];

    expect(describeCommand("codesign", args)).toBe(`codesign ${args.join(" ")}`);
  });

  it("keeps secrets out of the failure message thrown by run()", () => {
    const { script } = commandFixture("process.exit(3);\n");

    expect(() => run(process.execPath, [script, "--password", "hunter2"])).toThrow(/--password \*\*\*/);
    expect(() => run(process.execPath, [script, "--password", "hunter2"])).not.toThrow(/hunter2/);
  });
});
