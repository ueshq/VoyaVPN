import { describe, expect, it } from "vitest";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describeCommand, redactArgs, run } from "./common.mjs";

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
