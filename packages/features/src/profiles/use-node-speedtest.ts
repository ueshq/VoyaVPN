import { useState } from "react";

import { voyaCommands } from "@voya/client/transport";
import { speedtestPending, useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { SpeedtestResult, SpeedtestTarget } from "@voya/contracts";

import type { NodeOperation } from "./use-node-operation";

/** `source` names the button that started the run, so only it offers Stop. */
type SpeedtestRun = { ids: string[]; before: Record<string, SpeedtestResult>; source: string };

export function useNodeSpeedtest({ runOperation }: NodeOperation) {
  const speedtestRunning = useRuntimeEventStore(
    (state) => state.speedtestRunning,
  );
  const setSpeedtestRunning = useRuntimeEventStore(
    (state) => state.setSpeedtestRunning,
  );
  const results = useRuntimeEventStore((state) => state.speedtestResultsByProfileId);
  const [run, setRun] = useState<SpeedtestRun | null>(null);

  async function handleSpeedtest(target: SpeedtestTarget, source = "all") {
    if (useRuntimeEventStore.getState().speedtestRunning) return;
    setSpeedtestRunning(true);
    // Results replace their entries as they arrive, so a node counts as done
    // once its entry differs from the one before the run and has finished.
    setRun({
      before: useRuntimeEventStore.getState().speedtestResultsByProfileId,
      ids: target.profileIds,
      source,
    });
    try {
      await runOperation(() => voyaCommands().runSpeedtest({ target }));
    } finally {
      setSpeedtestRunning(false);
      setRun(null);
    }
  }

  async function handleCancelSpeedtest() {
    await runOperation(async () => {
      const status = await voyaCommands().cancelSpeedtest();
      setSpeedtestRunning(status.running);
    });
  }

  const speedtestProgress = run
    ? {
        done: run.ids.filter((id) => {
          const result = results[id];
          return result && result !== run.before[id] && !speedtestPending(result);
        }).length,
        total: run.ids.length,
      }
    : null;

  return {
    handleCancelSpeedtest,
    handleSpeedtest,
    speedtestProgress,
    speedtestRunning,
    speedtestSource: run?.source ?? null,
  };
}
