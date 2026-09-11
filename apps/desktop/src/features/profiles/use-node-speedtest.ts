import { cancelSpeedtest, runSpeedtest } from "@/ipc/commands";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { SpeedtestTarget } from "@/ipc/bindings";
import type { NodeOperation } from "./use-node-operation";

export function useNodeSpeedtest({ runOperation }: NodeOperation) {
  const speedtestRunning = useRuntimeEventStore(
    (state) => state.speedtestRunning,
  );
  const setSpeedtestRunning = useRuntimeEventStore(
    (state) => state.setSpeedtestRunning,
  );
  async function handleSpeedtest(target: SpeedtestTarget) {
    if (useRuntimeEventStore.getState().speedtestRunning) return;
    setSpeedtestRunning(true);
    try {
      await runOperation(() =>
        runSpeedtest({
          target,
        }),
      );
    } finally {
      setSpeedtestRunning(false);
    }
  }

  async function handleCancelSpeedtest() {
    await runOperation(async () => {
      const status = await cancelSpeedtest();
      useRuntimeEventStore.getState().setSpeedtestStatus(status);
    });
  }

  return { speedtestRunning, handleSpeedtest, handleCancelSpeedtest };
}
