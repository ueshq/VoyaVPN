import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { repoRootFromScript } from "../lib/common.mjs";
import {
  generateCommandNames,
  generateCommandWire,
  generateContractsSource,
  parseCommands,
  parseEvents,
} from "./contracts-source.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const bindings = readFileSync(resolve(repoRoot, "apps/desktop/src/ipc/bindings.ts"), "utf8");

function fixture({ commands, events, types }) {
  return [
    "/** Commands */",
    "export const commands = {",
    commands,
    "};",
    "",
    "/** Events */",
    "export const events = {",
    events,
    "};",
    "",
    "/* Types */",
    types,
    "",
    "/* Tauri Specta runtime */",
  ].join("\n");
}

describe("parseCommands", () => {
  it("unwraps the result envelope", () => {
    const parsed = parseCommands(
      '\tloadAppSettings: () => typedError<AppSettingsV1, AppError>(__TAURI_INVOKE("load_app_settings")),',
    );

    expect(parsed).toEqual([
      {
        doc: [],
        name: "loadAppSettings",
        params: [],
        parameters: "",
        returnType: "AppSettingsV1",
        wireName: "load_app_settings",
      },
    ]);
  });

  it("keeps a command whose return type spans several lines", () => {
    const block = [
      "\tpolicyGroupRuntime: () => typedError<{",
      "\tgroupId: string,",
      "\tmembers: PolicyGroupRuntimeMember[],",
      '} | null, AppError>(__TAURI_INVOKE("policy_group_runtime")),',
    ].join("\n");

    const [command] = parseCommands(block);

    expect(command.name).toBe("policyGroupRuntime");
    // Re-indented so the entry reads correctly once nested in `VoyaCommands`.
    expect(command.returnType).toBe(
      ["{", "\t\tgroupId: string,", "\t\tmembers: PolicyGroupRuntimeMember[],", "\t} | null"].join("\n"),
    );
  });

  it("carries each command's doc comment", () => {
    const block = [
      "\t/**  Returns an empty string when the clipboard holds no text. */",
      '\treadClipboardText: () => typedError<string, AppError>(__TAURI_INVOKE("read_clipboard_text")),',
    ].join("\n");

    expect(parseCommands(block)[0].doc).toEqual([
      "\t/**  Returns an empty string when the clipboard holds no text. */",
    ]);
  });

  it("refuses to silently drop a command it cannot parse", () => {
    // A dropped command would still compile and a platform binding would still
    // satisfy the contract, so the count has to be checked rather than trusted.
    const block = [
      '\tfine: () => typedError<string, AppError>(__TAURI_INVOKE("fine")),',
      '\tstrange: <T>() => typedError<string, AppError>(__TAURI_INVOKE("strange")),',
    ].join("\n");

    expect(() => parseCommands(block)).toThrow(/Parsed 1 commands but bindings.ts invokes 2/);
  });

  it("rejects a commands block it does not recognize", () => {
    expect(() => parseCommands("")).toThrow(/No commands parsed/);
  });
});

describe("parseEvents", () => {
  it("maps each channel to its payload type", () => {
    expect(parseEvents('\tappEvent: makeEvent<AppEvent>("app-event"),')).toEqual([
      { key: "appEvent", payloadType: "AppEvent", channel: "app-event" },
    ]);
  });

  it("refuses to silently drop an event", () => {
    const block = [
      '\tappEvent: makeEvent<AppEvent>("app-event"),',
      "\tweird: makeEvent<Foo | Bar>(someName),",
    ].join("\n");

    expect(() => parseEvents(block)).toThrow(/Parsed 1 events but bindings.ts declares 2/);
  });
});

describe("generateContractsSource", () => {
  it("emits commands, channels and types without any Tauri import", () => {
    const source = generateContractsSource(
      fixture({
        commands: '\trestartCore: () => typedError<RuntimeStatusResponse, AppError>(__TAURI_INVOKE("restart_core")),',
        events: '\tappEvent: makeEvent<AppEvent>("app-event"),',
        types: "export type AppEvent = { kind: 'notice' };",
      }),
    );

    expect(source).toContain("restartCore: () => Promise<RuntimeStatusResponse>,");
    expect(source).toContain('appEvent: { channel: "app-event"; payload: AppEvent },');
    expect(source).toContain("export type AppEvent = { kind: 'notice' };");
    expect(source).not.toContain("__TAURI");
    expect(source).not.toContain("typedError");
  });

  it("fails loudly when a marker it anchors on disappears", () => {
    expect(() => generateContractsSource("export const commands = {};")).toThrow(/missing the \/\*\* Commands \*\//);
  });

  it("reproduces the checked-in @voya/contracts source", () => {
    // The drift gate lives in bindings.mjs, which needs cargo; this keeps the
    // same guarantee inside the fast test suite.
    const committed = readFileSync(resolve(repoRoot, "packages/contracts/src/generated.ts"), "utf8");

    expect(generateContractsSource(bindings)).toBe(committed);
  });

  it("covers every command the real bindings expose", () => {
    const source = generateContractsSource(bindings);
    const invocations = bindings.split("__TAURI_INVOKE(").length - 1;

    expect(source.split("=> Promise<").length - 1).toBe(invocations);
  });
});

describe("generateCommandWire", () => {
  it("keeps the wire name and the argument names a transport has to rebuild", () => {
    const wire = generateCommandWire(
      fixture({
        commands: [
          '\tsetTunEnabled: (enabled: boolean) => typedError<TunStatus, AppError>(__TAURI_INVOKE("set_tun_enabled", { enabled })),',
          '\trestartCore: () => typedError<RuntimeStatusResponse, AppError>(__TAURI_INVOKE("restart_core")),',
        ].join("\n"),
        events: '\tappEvent: makeEvent<AppEvent>("app-event"),',
        types: "export type AppEvent = { kind: 'notice' };",
      }),
    );

    expect(wire).toContain('setTunEnabled: { name: "set_tun_enabled", params: ["enabled"] },');
    expect(wire).toContain('restartCore: { name: "restart_core", params: [] },');
    // `satisfies` is what makes a missing entry a compile error downstream.
    expect(wire).toContain("satisfies Record<keyof VoyaCommands, { name: string; params: readonly string[] }>");
  });

  it("refuses a command whose invocation disagrees with its parameters", () => {
    // A reordered object would send `{ a: b, b: a }` and only fail on device.
    const block = '\tmoveProfile: (id: string, action: MoveAction) => typedError<null, AppError>(__TAURI_INVOKE("move_profile", { action, id })),';

    expect(() => parseCommands(block)).toThrow(/declares \(id, action\) but invokes with \(action, id\)/);
  });

  it("reproduces the checked-in wire table", () => {
    const committed = readFileSync(resolve(repoRoot, "packages/contracts/src/commands.ts"), "utf8");

    expect(generateCommandWire(bindings)).toBe(committed);
  });
});

describe("generateCommandNames", () => {
  it("reproduces the checked-in command list, sorted and complete", () => {
    const committed = readFileSync(resolve(repoRoot, "packages/contracts/commands.json"), "utf8");
    const names = JSON.parse(committed);

    expect(generateCommandNames(bindings)).toBe(committed);
    expect(names).toEqual([...names].sort());
    expect(names).toHaveLength(bindings.split("__TAURI_INVOKE(").length - 1);
  });
});
