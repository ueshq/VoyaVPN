import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vite-plus/test";

import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { SpeedtestResult, SpeedtestRunResult } from "@voya/contracts";

import { installFakeCommands } from "../test/backend";
import { useNodeOperation } from "./use-node-operation";
import { useNodeSpeedtest } from "./use-node-speedtest";

const ipc = installFakeCommands({ runSpeedtest: vi.fn() });

function finished(indexId: string): SpeedtestResult {
  return { countryCode: null, delay: 40, detail: null, indexId, ipInfo: null, outcome: "completed" };
}

function arrive(...results: SpeedtestResult[]) {
  act(() => {
    useRuntimeEventStore.setState((state) => ({
      speedtestResultsByProfileId: {
        ...state.speedtestResultsByProfileId,
        ...Object.fromEntries(results.map((result) => [result.indexId, result])),
      },
    }));
  });
}

describe("useNodeSpeedtest", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    useRuntimeEventStore.setState({ speedtestResultsByProfileId: {}, speedtestRunning: false });
  });

  it("keeps counting when a saved node empties the results mid-run", async () => {
    let finish!: (result: SpeedtestRunResult) => void;
    ipc.runSpeedtest.mockReturnValue(
      new Promise<SpeedtestRunResult>((resolve) => {
        finish = resolve;
      }),
    );
    const { result } = renderHook(() => useNodeSpeedtest(useNodeOperation()));

    let run!: Promise<void>;
    act(() => {
      run = result.current.handleSpeedtest({ profileIds: ["a", "b", "c"], scope: "profiles" });
    });
    expect(result.current.speedtestProgress).toEqual({ done: 0, total: 3 });

    arrive(finished("a"), { ...finished("b"), outcome: "testing" });
    expect(result.current.speedtestProgress).toEqual({ done: 1, total: 3 });

    // Switching the node, or a subscription refresh, lands here.
    act(() => useRuntimeEventStore.getState().clearSpeedtestResults());
    expect(result.current.speedtestProgress).toEqual({ done: 1, total: 3 });

    arrive(finished("b"), finished("c"));
    expect(result.current.speedtestProgress).toEqual({ done: 3, total: 3 });

    await act(async () => {
      finish({ cancelled: false, completedCount: 3, selectedCount: 3 });
      await run;
    });
    await waitFor(() => expect(result.current.speedtestProgress).toBeNull());
  });
});
