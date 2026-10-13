import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vite-plus/test";

describe("design token gate", () => {
  let directory;
  let source;
  let script;

  beforeEach(() => {
    // Exercise the actual CLI in an isolated checkout, never mutate the
    // live palette while another frontend or Metro build is reading it.
    directory = mkdtempSync(join(tmpdir(), "voya-design-tokens-"));
    mkdirSync(join(directory, "scripts"));
    script = join(directory, "scripts/design-tokens.mjs");
    cpSync(new URL("./design-tokens.mjs", import.meta.url), script);
    source = join(directory, "packages/design-tokens");
    cpSync(new URL("../packages/design-tokens", import.meta.url), source, { recursive: true });
  });
  afterEach(() => rmSync(directory, { force: true, recursive: true }));

  const run = (...args) => spawnSync(process.execPath, [script, ...args], { encoding: "utf8" });
  const edit = (change) => {
    const path = join(source, "tokens.json");
    const tokens = JSON.parse(readFileSync(path, "utf8"));
    change(tokens);
    writeFileSync(path, JSON.stringify(tokens));
  };

  it("checks complete themes without writing the generated files", () => {
    const paths = ["browser.css", "system.css", "native.css", "tokens.ts"].map((file) => join(source, "src", file));
    const before = paths.map((path) => statSync(path, { bigint: true }).mtimeNs);
    expect(run("--check").status).toBe(0);
    expect(paths.map((path) => statSync(path, { bigint: true }).mtimeNs)).toEqual(before);
  });

  it("rejects an incomplete theme", () => {
    edit((tokens) => delete tokens.colors.dark.muted);
    const result = run("--check");
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("both themes must provide every color role");
  });

  it("rejects low-contrast semantic text before generating output", () => {
    edit((tokens) => (tokens.colors.light.muted = tokens.colors.light.canvas));
    const result = run();
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("light.muted/canvas");
  });

  it("rejects output drift and repairs it only through generation", () => {
    const path = join(source, "src/native.css");
    const original = readFileSync(path, "utf8");
    writeFileSync(path, original + "/* drift */\n");
    const result = run("--check");
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("Token output drift");
    expect(readFileSync(path, "utf8")).not.toBe(original);
    expect(run().status).toBe(0);
    expect(readFileSync(path, "utf8")).toBe(original);
    expect(run("--check").status).toBe(0);
  });
});
