import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { changeLocale } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import type { RuntimeStatusResponse } from "@/ipc/bindings";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { makeAppSettings } from "@voya/features/settings/app-settings.test-fixture";

import { ExitIpMetric } from "./exit-ip-metric";

const ipc = vi.hoisted(() => ({ checkConnectionIp: vi.fn(), loadAppSettings: vi.fn() }));
vi.mock("@/ipc/commands", () => ipc);

const connected: RuntimeStatusResponse = {
  activeProfileId: "tokyo",
  activeTunBackend: null,
  connectedDurationMs: 0,
  mainPid: 42,
  prePid: null,
  state: "connected",
};

function Metric() {
  const { t } = useI18n();
  return <dl><ExitIpMetric t={t} /></dl>;
}

function renderMetric() {
  return renderWithQuery(<Metric />);
}

describe("ExitIpMetric", () => {
  beforeEach(async () => {
    vi.resetAllMocks();
    await changeLocale("en", { persist: false });
    useRuntimeEventStore.setState({ coreState: null });
    ipc.loadAppSettings.mockResolvedValue(makeAppSettings());
    ipc.checkConnectionIp.mockResolvedValue({ countryCode: "JP", ip: "203.0.113.9" });
  });

  it("offers no lookup while disconnected", () => {
    renderMetric();

    expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("Not checked");
    expect(screen.getByRole("button", { name: "Check exit IP" })).toBeDisabled();
    expect(ipc.checkConnectionIp).not.toHaveBeenCalled();
  });

  it("checks on request and shows the address with its country", async () => {
    const user = userEvent.setup();
    useRuntimeEventStore.setState({ coreState: connected });
    renderMetric();

    expect(ipc.checkConnectionIp).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Check exit IP" }));

    await waitFor(() =>
      expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("203.0.113.9 · JP"),
    );
  });

  it("remembers the address when the screen unmounts and remounts", async () => {
    const user = userEvent.setup();
    useRuntimeEventStore.setState({ coreState: connected });
    // One query client across both mounts: the cache outlives the screen.
    const queryClient = createTestQueryClient();
    const view = renderWithQuery(<Metric />, { queryClient });

    await user.click(screen.getByRole("button", { name: "Check exit IP" }));
    await waitFor(() =>
      expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("203.0.113.9 · JP"),
    );

    view.unmount();
    renderWithQuery(<Metric />, { queryClient });

    expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("203.0.113.9 · JP");
    expect(ipc.checkConnectionIp).toHaveBeenCalledOnce();
  });

  it("checks automatically on connect when enabled and reports a failure", async () => {
    const settings = makeAppSettings();
    settings.behavior.autoCheckIp = true;
    ipc.loadAppSettings.mockResolvedValue(settings);
    ipc.checkConnectionIp.mockRejectedValue(new Error("timed out"));
    useRuntimeEventStore.setState({ coreState: connected });
    renderMetric();

    await waitFor(() => expect(ipc.checkConnectionIp).toHaveBeenCalledOnce());
    await waitFor(() =>
      expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("Check failed"),
    );
  });
});
