import { describe, expect, it } from "vitest";

import { parseArgs } from "./args.mjs";

const spec = {
  "--input": { key: "input" },
  "--out|--output": { key: "output" },
  "--timeout-ms": { key: "timeoutMs", parse: (value) => Number.parseInt(value, 10) },
  "--target": { key: "targets", list: true },
  "--allow-empty": { key: "allowEmpty", value: true },
  "--download-and-hash": { key: "downloadAndHash", value: true, also: { probe: true } },
  "--skip-release-index": { key: "releaseIndex", value: null },
};

const defaults = { allowEmpty: false, input: null, output: "dist/release", probe: false, releaseIndex: "index.json" };

describe("shared CLI argument reader", () => {
  it("reads strings, aliases, lists, parsed values and constants", () => {
    expect(
      parseArgs(
        [
          "--input",
          "bundle",
          "--output",
          "out",
          "--target",
          "darwin-x86_64,linux-x86_64",
          "--target",
          "windows-x86_64",
          "--timeout-ms",
          "2500",
          "--allow-empty",
        ],
        spec,
        defaults,
      ),
    ).toEqual({
      allowEmpty: true,
      input: "bundle",
      output: "out",
      probe: false,
      releaseIndex: "index.json",
      targets: ["darwin-x86_64", "linux-x86_64", "windows-x86_64"],
      timeoutMs: 2500,
    });

    expect(parseArgs(["--out", "alias"], spec, defaults).output).toBe("alias");
  });

  it("applies implied options and null constants", () => {
    const options = parseArgs(["--download-and-hash", "--skip-release-index"], spec, defaults);

    expect(options.downloadAndHash).toBe(true);
    expect(options.probe).toBe(true);
    expect(options.releaseIndex).toBeNull();
  });

  it("rejects unknown flags and missing values, and always accepts --help", () => {
    expect(() => parseArgs(["--nope"], spec, defaults)).toThrow("Unknown argument: --nope");
    expect(() => parseArgs(["--input"], spec, defaults)).toThrow("--input requires a value");
    expect(() => parseArgs(["--input", "--output", "out"], spec, defaults)).toThrow("--input requires a value");
    expect(parseArgs(["--help"], spec, defaults).help).toBe(true);
    expect(parseArgs(["-h"], spec, defaults).help).toBe(true);
  });

  it("accepts the --flag=value form", () => {
    expect(parseArgs(["--input=bundle", "--timeout-ms=2500"], spec, defaults)).toMatchObject({
      input: "bundle",
      timeoutMs: 2500,
    });
  });
});
