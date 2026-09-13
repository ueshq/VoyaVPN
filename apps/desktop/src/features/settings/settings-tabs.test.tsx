import { useState } from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { AppSettingsV1, AppearanceSettings } from "@/ipc/bindings";

import { makeAppSettings } from "./app-settings.test-fixture";
import { CoreTab } from "./core-tab";
import { GeneralTab } from "./general-tab";
import { NetworkTab } from "./network-tab";
import { TestsTab } from "./tests-tab";
import type {
  AppSettingsController,
  AppSettingsFormController,
} from "./use-app-settings";

type SettingsTab = (props: {
  controller: AppSettingsFormController;
}) => React.ReactNode;

describe("semantic settings tabs", () => {
  beforeEach(() => {
    useRuntimeEventStore.setState({ coreState: null, tun: null });
    useRuntimeEventStore
      .getState()
      .setSysProxy({
        management: "automatic",
        observation: "unknown",
        manualCleanupRequired: false,
        requestedMode: "forcedClear",
        effectiveMode: "forcedClear",
        proxy: null,
        exceptions: "",
      });
  });
  it("shows manual setup without a copy action while disconnected", () => {
    const status = useRuntimeEventStore.getState().sysProxy!;
    useRuntimeEventStore
      .getState()
      .setSysProxy({ ...status, management: "manual" });
    render(<TabHarness Component={NetworkTab} />);
    expect(screen.getByText("Manual proxy setup")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Copy address" }),
    ).not.toBeInTheDocument();
  });

  it("keeps manual proxy addresses and observations live while the network tab is open", () => {
    useRuntimeEventStore.getState().setSysProxy({
      ...useRuntimeEventStore.getState().sysProxy!,
      management: "manual",
      requestedMode: "forcedChange",
      proxy: "127.0.0.1:10808",
    });
    render(<TabHarness Component={NetworkTab} />);
    expect(
      screen.queryByRole("button", { name: "Copy address" }),
    ).not.toBeInTheDocument();
    act(() =>
      useRuntimeEventStore.getState().setCoreState({
        activeProfileId: "node",
        activeTunBackend: null,
        connectedDurationMs: 0,
        mainPid: 42,
        prePid: null,
        runningCoreType: "singBox",
        state: "connected",
      }),
    );
    expect(
      screen.getByText(/HTTP \/ HTTPS \/ SOCKS: 127\.0\.0\.1:10808/),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Copy address" })).toBeEnabled();
    act(() =>
      useRuntimeEventStore.getState().setSysProxy({
        ...useRuntimeEventStore.getState().sysProxy!,
        observation: "clear",
      }),
    );
    expect(screen.getByRole("status")).toHaveTextContent(
      "No enabled system proxy was found.",
    );
    act(() =>
      useRuntimeEventStore.getState().setCoreState({
        ...useRuntimeEventStore.getState().coreState!,
        state: "disconnected",
      }),
    );
    expect(
      screen.queryByRole("button", { name: "Copy address" }),
    ).not.toBeInTheDocument();
  });

  it("hides manual setup on automatically managed platforms", () => {
    render(<TabHarness Component={NetworkTab} />);
    expect(screen.queryByTestId("manual-proxy-panel")).not.toBeInTheDocument();
  });
  it("updates all core, multiplexing, and Hysteria controls", async () => {
    const user = userEvent.setup();
    const { container } = render(<TabHarness Component={CoreTab} />);

    for (const checkbox of screen.getAllByRole("checkbox"))
      await user.click(checkbox);
    for (const input of container.querySelectorAll<HTMLInputElement>(
      'input:not([type="checkbox"])',
    )) {
      fireEvent.change(input, {
        target: { value: input.inputMode === "numeric" ? "" : "changed" },
      });
      if (input.inputMode === "numeric")
        fireEvent.change(input, { target: { value: "12" } });
      fireEvent.blur(input);
    }

    expect(container.querySelector("#rt-user-agent")).toHaveValue("changed");
    expect(container.querySelector("#rt-hysteria-up")).toHaveValue("12");
  });

  it("reveals the fallback delay only for ClientHello fragmentation", async () => {
    const user = userEvent.setup();
    const { container } = render(<TabHarness Component={CoreTab} />);

    expect(
      container.querySelector("#rt-fragment-fallback-delay"),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("combobox", { name: "TLS fragmentation" }));
    await user.click(
      await screen.findByRole("option", { name: "Split ClientHello" }),
    );
    expect(container.querySelector("#rt-fragment-fallback-delay")).toHaveValue(
      "500",
    );
  });

  it("updates TUN and system proxy controls", async () => {
    const user = userEvent.setup();
    const { container } = render(<TabHarness Component={NetworkTab} />);

    for (const checkbox of screen.getAllByRole("checkbox"))
      await user.click(checkbox);
    for (const input of container.querySelectorAll<HTMLInputElement>(
      'input:not([type="checkbox"])',
    )) {
      fireEvent.change(input, {
        target: { value: input.inputMode === "numeric" ? "1500" : " value " },
      });
      fireEvent.blur(input);
    }

    expect(container.querySelector("#rt-tun-mtu")).toHaveValue("1500");
    expect(container.querySelector("#rt-sysproxy-exceptions")).toHaveValue(
      " value ",
    );
  });

  it("updates every speed-test setting and handles empty numbers", () => {
    const { container } = render(<TabHarness Component={TestsTab} />);

    for (const input of container.querySelectorAll<HTMLInputElement>("input")) {
      fireEvent.change(input, {
        target: {
          value:
            input.inputMode === "numeric" ? "" : "https://new.example.test",
        },
      });
      if (input.inputMode === "numeric")
        fireEvent.change(input, { target: { value: "25" } });
      fireEvent.blur(input);
    }

    expect(screen.getByLabelText("Speed Ping Test URL")).toHaveValue(
      "https://new.example.test",
    );
    expect(container.querySelector("#rt-speedtest-timeout")).toHaveValue("25");
    expect(screen.queryByLabelText("Speed Test URL")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("UDP Test Url")).not.toBeInTheDocument();
  });

  it("selects the active UI language when the stored language was removed", () => {
    const settings = makeAppSettings();
    settings.appearance.language = "fa";
    render(
      <GeneralTab controller={{ ...emptyController(false, null), settings }} />,
    );

    expect(screen.getByRole("button", { name: "English" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "简体中文" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(screen.getByRole("button", { name: "繁體中文" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("updates appearance and autostart", async () => {
    const user = userEvent.setup();
    render(<TabHarness Component={GeneralTab} />);

    await user.click(screen.getByRole("button", { name: "Dark" }));
    expect(screen.getByRole("button", { name: "Dark" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await user.click(screen.getByRole("button", { name: "简体中文" }));
    expect(screen.getByRole("button", { name: "简体中文" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    const autostart = screen.getByRole("checkbox", { name: "Autostart" });
    expect(autostart).not.toBeChecked();
    await user.click(autostart);
    expect(autostart).toBeChecked();
  });
});

function TabHarness({ Component }: { Component: SettingsTab }) {
  const [settings, setSettings] = useState(makeAppSettings());
  const controller: AppSettingsFormController = {
    error: null,
    fieldErrors: {},
    saved: false,
    saving: false,
    retry: vi.fn(),
    setAppearance: (appearance: AppearanceSettings) => {
      setSettings((current) => ({ ...current, appearance }));
    },
    settings,
    update: setSettings,
    working: false,
  };
  return <Component controller={controller} />;
}

function emptyController(
  working: boolean,
  error: string | null,
): AppSettingsController {
  return {
    error,
    fieldErrors: {},
    saved: false,
    saving: false,
    retry: vi.fn(),
    setAppearance: vi.fn(),
    settings: null,
    update:
      vi.fn<(updater: (current: AppSettingsV1) => AppSettingsV1) => void>(),
    working,
  };
}
