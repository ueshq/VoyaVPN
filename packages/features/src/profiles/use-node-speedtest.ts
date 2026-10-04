import { useState } from "react";

import { voyaCommands } from "@voya/client/transport";
import { speedtestPending, useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { SpeedtestResult, SpeedtestTarget } from "@voya/contracts";

import type { NodeOperation } from "./use-node-operation";

/** `source` names the button that started the run, so only it offers Stop. */
type SpeedtestRun = { done: number; source: string; total: number };

/**
 * Counts a run's nodes as they finish, in a set of its own. The live results
 * are no place to count from: saving any node empties them (the measurements
 * no longer describe what is stored), and a count read back from them would
 * fall to zero mid-run and never reach the total.
 */
function watchProgress(ids: string[], onDone: (done: number) => void) {
  const before = useRuntimeEventStore.getState().speedtestResultsByProfileId;
  const waiting = new Set(ids);
  return useRuntimeEventStore.subscribe((state, previous) => {
    const results = state.speedtestResultsByProfileId;
    if (results === previous.speedtestResultsByProfileId) return;
    const left = waiting.size;
    for (const id of waiting) {
      // Results replace their entries as they arrive, so a node is done once
      // its entry differs from the one before the run and has finished.
      const result: SpeedtestResult | undefined = results[id];
      if (result && result !== before[id] && !speedtestPending(result)) waiting.delete(id);
    }
    if (waiting.size !== left) onDone(ids.length - waiting.size);
  });
}

export function useNodeSpeedtest({ runOperation }: NodeOperation) {
  const speedtestRunning = useRuntimeEventStore(
    (state) => state.speedtestRunning,
  );
  const setSpeedtestRunning = useRuntimeEventStore(
    (state) => state.setSpeedtestRunning,
  );
  const [run, setRun] = useState<SpeedtestRun | null>(null);

  async function handleSpeedtest(target: SpeedtestTarget, source = "all") {
    if (useRuntimeEventStore.getState().speedtestRunning) return;
    setSpeedtestRunning(true);
    setRun({ done: 0, source, total: target.profileIds.length });
    const stopWatching = watchProgress(target.profileIds, (done) => {
      setRun((current) => current && { ...current, done });
    });
    try {
      await runOperation(() => voyaCommands().runSpeedtest({ target }));
    } finally {
      stopWatching();
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

  const speedtestProgress = run ? { done: run.done, total: run.total } : null;

  return {
    handleCancelSpeedtest,
    handleSpeedtest,
    speedtestProgress,
    speedtestRunning,
    speedtestSource: run?.source ?? null,
  };
}
