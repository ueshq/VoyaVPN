import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { AppSettingsV1, AppearanceSettings } from "@/ipc/bindings";

import { makeAppSettings } from "./app-settings.test-fixture";
import { CoreTab } from "./core-tab";
import { GeneralTab } from "./general-tab";
import { NetworkTab } from "./network-tab";
import { TestsTab } from "./tests-tab";
import type { AppSettingsController } from "./use-app-settings";

type SettingsTab = (props: { controller: AppSettingsController }) => React.ReactNode;

describe("semantic settings tabs", () => {
  beforeEach(() => {
    useRuntimeEventStore.getState().setSysProxy({ management: "automatic", observation: "unknown", manualCleanupRequired: false,
      requestedMode: "forcedClear", effectiveMode: "forcedClear", pacAvailable: true, proxy: null, pacUrl: null, exceptions: "" });
  });
  it("hides custom scripts when system proxy management is manual", () => {
    const status = useRuntimeEventStore.getState().sysProxy!;
    useRuntimeEventStore.getState().setSysProxy({ ...status, management: "manual" });
    const { container } = render(<TabHarness Component={NetworkTab} />);
    expect(container.querySelector("#rt-sysproxy-script-path")).not.toBeInTheDocument();
  });
  it.each([
    [CoreTab, "Loading"],
    [NetworkTab, "Loading"],
    [TestsTab, "Loading"],
    [GeneralTab, "Loading"],
  ] as Array<[SettingsTab, string]>)("renders the pending %p state", (Component, label) => {
    render(<Component controller={emptyController(true, null)} />);
    expect(screen.getByText(label)).toBeInTheDocument();
  });

  it.each([CoreTab, NetworkTab, TestsTab, GeneralTab] as SettingsTab[])(
    "renders the failed %p state",
    (Component) => {
      render(<Component controller={emptyController(false, "settings failed")} />);
      expect(screen.getByText("settings failed")).toBeInTheDocument();
    },
  );

  it("updates all core, multiplexing, and Hysteria controls", async () => {
    const user = userEvent.setup();
    const { container } = render(<TabHarness Component={CoreTab} />);

    for (const checkbox of screen.getAllByRole("checkbox")) await user.click(checkbox);
    for (const input of container.querySelectorAll<HTMLInputElement>('input:not([type="checkbox"])')) {
      fireEvent.change(input, { target: { value: input.type === "number" ? "" : "changed" } });
      if (input.type === "number") fireEvent.change(input, { target: { value: "12" } });
    }

    expect(container.querySelector("#rt-user-agent")).toHaveValue("changed");
    expect(container.querySelector("#rt-hysteria-up")).toHaveValue(12);
  });

  it("updates TUN and system proxy controls including nullable paths", async () => {
    const user = userEvent.setup();
    const { container } = render(<TabHarness Component={NetworkTab} />);

    for (const checkbox of screen.getAllByRole("checkbox")) await user.click(checkbox);
    for (const input of container.querySelectorAll<HTMLInputElement>('input:not([type="checkbox"])')) {
      fireEvent.change(input, { target: { value: input.type === "number" ? "" : " value " } });
    }

    expect(container.querySelector("#rt-tun-mtu")).toHaveValue(1500);
    expect(container.querySelector("#rt-sysproxy-pac-path")).toHaveValue(" value ");
    expect(container.querySelector("#rt-sysproxy-script-path")).toHaveValue(" value ");
  });

  it("updates every speed-test setting and handles empty numbers", () => {
    const { container } = render(<TabHarness Component={TestsTab} />);

    for (const input of container.querySelectorAll<HTMLInputElement>("input")) {
      fireEvent.change(input, { target: { value: input.type === "number" ? "" : "https://new.example.test" } });
      if (input.type === "number") fireEvent.change(input, { target: { value: "25" } });
    }

    expect(screen.getByLabelText("Speed Ping Test URL")).toHaveValue("https://new.example.test");
    expect(container.querySelector("#rt-speedtest-timeout")).toHaveValue(25);
    expect(screen.getByLabelText("Proxy group latency test concurrency")).toHaveValue(25);
    expect(screen.queryByLabelText("Speed Test URL")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("UDP Test Url")).not.toBeInTheDocument();
  });

  it("selects the active UI language when the stored language was removed", () => {
    const settings = makeAppSettings();
    settings.appearance.language = "fa";
    render(<GeneralTab controller={{ ...emptyController(false, null), settings }} />);

    expect(screen.getByRole("button", { name: "EN" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "简" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("button", { name: "繁" })).toHaveAttribute("aria-pressed", "false");
  });

  it("updates appearance, behavior, and the single shortcut contract", async () => {
    const user = userEvent.setup();
    render(<TabHarness Component={GeneralTab} />);

    await user.click(screen.getByRole("button", { name: "Dark" }));
    await user.click(screen.getByRole("button", { name: "简" }));
    await user.click(screen.getByRole("checkbox", { name: "Autostart" }));
    await user.click(screen.getByRole("button", { name: "Ctrl" }));
    await user.click(screen.getByRole("button", { name: "Alt" }));
    await user.click(screen.getByRole("button", { name: "Shift" }));

    const hotkey = screen.getByLabelText("Hotkey key");
    // A bare modifier is never recorded; the previously stored key stays.
    fireEvent.keyDown(hotkey, { code: "AltLeft", key: "Alt" });
    expect(hotkey).toHaveValue("V");
    // fireEvent returns false when the handler called preventDefault.
    expect(fireEvent.keyDown(hotkey, { code: "KeyA", key: "a" })).toBe(false);
    expect(hotkey).toHaveValue("A");
    fireEvent.keyDown(hotkey, { code: "F1", key: "F1" });
    expect(hotkey).toHaveValue("F1");
    fireEvent.keyDown(hotkey, { code: "Backspace", key: "Backspace" });
    expect(hotkey).toHaveValue("Backspace");
    fireEvent.keyDown(hotkey, { code: "MediaPlayPause", key: "MediaPlayPause" });
    expect(hotkey).toHaveValue("Backspace");
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(hotkey).toHaveValue("");
  });

  it("never traps the keyboard inside the hotkey capture field", async () => {
    const user = userEvent.setup();
    render(<TabHarness Component={GeneralTab} />);

    const hotkey = screen.getByLabelText("Hotkey key");
    fireEvent.keyDown(hotkey, { code: "KeyB", key: "b" });
    expect(hotkey).toHaveValue("B");

    // Tab and Escape stay available as focus navigation and cancel: neither is
    // recorded, and neither is swallowed (fireEvent returns true).
    expect(fireEvent.keyDown(hotkey, { code: "Tab", key: "Tab" })).toBe(true);
    expect(fireEvent.keyDown(hotkey, { code: "Tab", key: "Tab", shiftKey: true })).toBe(true);
    expect(fireEvent.keyDown(hotkey, { code: "Escape", key: "Escape" })).toBe(true);
    expect(hotkey).toHaveValue("B");

    hotkey.focus();
    await user.tab();
    expect(hotkey).not.toHaveFocus();
  });
});

function TabHarness({ Component }: { Component: SettingsTab }) {
  const [settings, setSettings] = useState(makeAppSettings());
  const controller: AppSettingsController = {
    dirty: false,
    discard: async () => undefined,
    error: null,
    fieldErrors: {},
    reload: async () => undefined,
    save: async () => true,
    saved: false,
    setAppearance: (appearance: AppearanceSettings) => {
      setSettings((current) => ({ ...current, appearance }));
    },
    settings,
    update: setSettings,
    working: false,
  };
  return <Component controller={controller} />;
}

function emptyController(working: boolean, error: string | null): AppSettingsController {
  return {
    dirty: false,
    discard: vi.fn(),
    error,
    fieldErrors: {},
    reload: vi.fn(),
    save: vi.fn(),
    saved: false,
    setAppearance: vi.fn(),
    settings: null,
    update: vi.fn<(updater: (current: AppSettingsV1) => AppSettingsV1) => void>(),
    working,
  };
}
