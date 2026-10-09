import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";

import { cachedTasks, commands, commandsUnder, groups, resolveCommand } from "./commands.mjs";
import { repoRootFromScript } from "./lib/common.mjs";
import viteConfig from "../vite.config.ts";

const repoRoot = repoRootFromScript(import.meta.url);
const packageScripts = JSON.parse(readFileSync(resolve(repoRoot, "package.json"), "utf8")).scripts;

describe("command resolution", () => {
  it("prefers the longest command name", () => {
    expect(resolveCommand(["native", "macos", "tunnel", "verify"])?.name).toBe("native macos tunnel verify");
    expect(resolveCommand(["native", "macos", "tunnel"])?.name).toBe("native macos tunnel");
    expect(resolveCommand(["build", "mac", "local"])?.name).toBe("build mac local");
    expect(resolveCommand(["build", "mac"])?.name).toBe("build mac");
  });

  it("leaves the remaining words to the command", () => {
    expect(resolveCommand(["native", "macos", "ne", "doctor", "--fix", "--app", "/tmp/a.app"])).toMatchObject({
      name: "native macos ne doctor",
      args: ["--fix", "--app", "/tmp/a.app"],
    });
    expect(resolveCommand(["native", "mobile", "rust", "ios", "--slice", "device"])?.args).toEqual([
      "--slice",
      "device",
    ]);
  });

  it("does not read a flag's value as part of the name", () => {
    expect(resolveCommand(["build", "mac", "--flavor", "local"])).toMatchObject({
      name: "build mac",
      args: ["--flavor", "local"],
    });
  });

  it("drops the separator between a command and its arguments", () => {
    expect(resolveCommand(["dev", "ios", "--", "--simulator", "iPhone 17"])?.args).toEqual([
      "--simulator",
      "iPhone 17",
    ]);
  });

  it("rejects words that name no command", () => {
    expect(resolveCommand(["build", "nope"])).toBeNull();
    expect(resolveCommand(["check"])).toBeNull();
    expect(resolveCommand([])).toBeNull();
  });

  it("lists the commands of a group for the usage message", () => {
    expect(commandsUnder(["build", "nope"])).toEqual(Object.keys(commands).filter((name) => name.startsWith("build ")));
    expect(commandsUnder(["nope"])).toEqual(Object.keys(commands));
  });
});

describe("command table", () => {
  it("names every command in lowercase words without a colon", () => {
    for (const name of Object.keys(commands)) expect(name).toMatch(/^[a-z0-9-]+( [a-z0-9-]+)+$/u);
  });

  it("runs only programs the dispatcher can spawn without a shell", () => {
    for (const [name, { steps }] of Object.entries(commands)) {
      expect(steps.length, name).toBeGreaterThan(0);
      for (const [program] of steps) expect(["node", "cargo", "vp"], name).toContain(program);
    }
  });

  it("passes a sub-command word through to a script that reads it", () => {
    expect(resolveCommand(["native", "windows", "tunnel", "install"])).toMatchObject({
      name: "native windows tunnel",
      args: ["install"],
    });
  });

  it("has one root script per first word, pointing at the dispatcher", () => {
    for (const group of groups) expect(packageScripts[group], group).toBe(`node scripts/run.mjs ${group}`);
  });

  it("points every node step at a script that exists", () => {
    for (const [name, { steps }] of Object.entries(commands)) {
      for (const [program, [script]] of steps) {
        if (program === "node") expect(() => readFileSync(resolve(repoRoot, script)), name).not.toThrow();
      }
    }
  });

  // vp refuses a task defined both in vite.config.ts and in package.json, and
  // a cached command with no task behind it would fail only when it is run.
  it("backs every cached command with a task in vite.config.ts", () => {
    const tasks = Object.keys(viteConfig.run.tasks);

    expect([...tasks].sort()).toEqual(Object.values(cachedTasks).sort());
    for (const task of tasks) expect(Object.keys(packageScripts)).not.toContain(task);
    for (const [name, task] of Object.entries(cachedTasks)) {
      expect(commands[name].steps).toEqual([["vp", ["run", task]]]);
    }
  });
});
