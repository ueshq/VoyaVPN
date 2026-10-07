import { createTestQueryClient, renderWithQuery } from "../test/render";
import { act, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { changeLocale } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import type { RuntimeStatusResponse } from "@voya/contracts";
import { queryKeys } from "@voya/client/query-keys";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { installFakeCommands } from "../test/backend";
import { exitIpLabel } from "./exit-ip-label";
import { useConnectionIp } from "./use-connection-ip";

const ipc = installFakeCommands({ checkConnectionIp: vi.fn() });

const connected: RuntimeStatusResponse = {
  activeProfileId: "tokyo",
  activeTunBackend: null,
  connectedDurationMs: 0,
  mainPid: 42,
  state: "connected",
};

/** What both Home screens render: the lookup, put into words. */
function Metric() {
  const { t } = useI18n();
  return <span data-testid="home-exit-ip">{exitIpLabel(useConnectionIp(), t)}</span>;
}

function renderMetric() {
  return renderWithQuery(<Metric />);
}

describe("useConnectionIp", () => {
  beforeEach(async () => {
    vi.resetAllMocks();
    await changeLocale("en", { persist: false });
    useRuntimeEventStore.setState({ coreState: null });
    ipc.checkConnectionIp.mockResolvedValue({ countryCode: "JP", ip: "203.0.113.9" });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("looks nothing up while disconnected", () => {
    renderMetric();

    expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("—");
    expect(ipc.checkConnectionIp).not.toHaveBeenCalled();
  });

  it("checks on connect and shows the address with its country", async () => {
    useRuntimeEventStore.setState({ coreState: connected });
    renderMetric();

    await waitFor(() =>
      expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("203.0.113.9 · JP"),
    );
  });

  it("remembers the address when the screen unmounts and remounts", async () => {
    useRuntimeEventStore.setState({ coreState: connected });
    // One query client across both mounts: the cache outlives the screen.
    const queryClient = createTestQueryClient();
    const view = renderWithQuery(<Metric />, { queryClient });

    await waitFor(() =>
      expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("203.0.113.9 · JP"),
    );

    view.unmount();
    renderWithQuery(<Metric />, { queryClient });

    expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("203.0.113.9 · JP");
    expect(ipc.checkConnectionIp).toHaveBeenCalledOnce();
  });

  it("keeps one result: a new connection checks again and drops the previous one's", async () => {
    useRuntimeEventStore.setState({ coreState: connected });
    const queryClient = createTestQueryClient();
    renderWithQuery(<Metric />, { queryClient });
    await waitFor(() =>
      expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("203.0.113.9 · JP"),
    );

    // A reconnect is a new core process, so a new key.
    ipc.checkConnectionIp.mockResolvedValue({ countryCode: "SG", ip: "198.51.100.7" });
    act(() => {
      useRuntimeEventStore.setState({ coreState: { ...connected, mainPid: 43 } });
    });

    await waitFor(() =>
      expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("198.51.100.7 · SG"),
    );
    const cached = queryClient.getQueryCache().findAll({ queryKey: queryKeys.connectionIp });
    expect(cached.map((query) => query.queryKey[1])).toEqual(["tokyo:43:0"]);
  });

  it("checks again when the same node reconnects with no process to tell them apart", async () => {
    // A core inside the system's tunnel provider has no process id, and the
    // reconnect below happens with Home unmounted — from the tray, say.
    const inProvider = { ...connected, mainPid: null };
    const { setCoreState } = useRuntimeEventStore.getState();
    act(() => setCoreState(inProvider));
    const queryClient = createTestQueryClient();
    const view = renderWithQuery(<Metric />, { queryClient });
    await waitFor(() =>
      expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("203.0.113.9 · JP"),
    );

    view.unmount();
    act(() => {
      setCoreState({ ...inProvider, state: "disconnected" });
      setCoreState(inProvider);
    });
    renderWithQuery(<Metric />, { queryClient });

    await waitFor(() => expect(ipc.checkConnectionIp).toHaveBeenCalledTimes(2));
  });

  it("tries a failed lookup again before reporting the failure", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    ipc.checkConnectionIp.mockRejectedValue(new Error("timed out"));
    useRuntimeEventStore.setState({ coreState: connected });
    renderMetric();

    await waitFor(() => expect(ipc.checkConnectionIp).toHaveBeenCalledOnce());
    expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("Checking…");

    await act(() => vi.advanceTimersByTimeAsync(10_000));

    expect(ipc.checkConnectionIp).toHaveBeenCalledTimes(3);
    expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("Check failed");
  });

  it("starts a failed lookup over on its own, and leaves an answer alone", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    ipc.checkConnectionIp.mockRejectedValue(new Error("timed out"));
    useRuntimeEventStore.setState({ coreState: connected });
    renderMetric();
    await act(() => vi.advanceTimersByTimeAsync(10_000));
    expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("Check failed");

    // The tunnel routes now; nobody has to leave Home and come back.
    ipc.checkConnectionIp.mockResolvedValue({ countryCode: "JP", ip: "203.0.113.9" });
    await act(() => vi.advanceTimersByTimeAsync(30_000));

    expect(screen.getByTestId("home-exit-ip")).toHaveTextContent("203.0.113.9 · JP");
    expect(ipc.checkConnectionIp).toHaveBeenCalledTimes(4);

    await act(() => vi.advanceTimersByTimeAsync(120_000));
    expect(ipc.checkConnectionIp).toHaveBeenCalledTimes(4);
  });
});
