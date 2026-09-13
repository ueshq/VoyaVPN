import { useState } from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { AppSettingsV1, AppearanceSettings, TunStatus } from "@/ipc/bindings";

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
        requestedMode: "forcedClear",
        effectiveMode: "forcedClear",
        proxy: null,
        exceptions: "",
      });
  });
  it("offers capture mode, system proxy and per-app proxy only where the platform has them", () => {
    const { unmount } = render(<TabHarness Component={NetworkTab} />);
    expect(screen.getByRole("group", { name: "Traffic capture" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "System proxy" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Per-app proxy" })).toBeInTheDocument();
    unmount();

    act(() => {
      useRuntimeEventStore.getState().setSysProxy({
        ...useRuntimeEventStore.getState().sysProxy!,
        management: "unsupported",
      });
      useRuntimeEventStore.setState({ tun: macosTun });
    });
    render(<TabHarness Component={NetworkTab} />);

    expect(screen.getByRole("heading", { name: "TUN" })).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Traffic capture" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "System proxy" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Per-app proxy" })).not.toBeInTheDocument();
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
    expect(container.querySelector("#rt-inbound-port")).toHaveValue("1500");
    expect(container.querySelector("#rt-sysproxy-exceptions")).toHaveValue(
      " value ",
    );
  });

  it("reveals LAN credentials only behind a separate LAN port", async () => {
    const user = userEvent.setup();
    const { container } = render(<TabHarness Component={NetworkTab} />);

    expect(
      screen.queryByRole("checkbox", { name: "Separate LAN port" }),
    ).not.toBeInTheDocument();
    await user.click(
      screen.getByRole("checkbox", { name: "Allow connections from the LAN" }),
    );
    expect(container.querySelector("#rt-inbound-password")).not.toBeInTheDocument();
    await user.click(screen.getByRole("checkbox", { name: "Separate LAN port" }));

    expect(container.querySelector("#rt-inbound-username")).toBeInTheDocument();
    expect(container.querySelector("#rt-inbound-password")).toHaveAttribute(
      "type",
      "password",
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

const macosTun: TunStatus = {
  allowEnableTun: true,
  backend: "macosPacketTunnel",
  elevationGranted: false,
  enabled: true,
  expectedProviderPath: null,
  lastProviderError: null,
  nativeComponentReady: true,
  needsServiceInstall: false,
  needsVpnPermission: false,
  preflight: { notes: [], platform: "macos", routeRestoreNote: "", state: "ready", windowsCleanupDevices: [] },
  providerPathMismatch: false,
  providerState: "stopped",
  requiresElevation: false,
  resolvedProviderPath: null,
  restoreOnDisconnect: true,
};

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
