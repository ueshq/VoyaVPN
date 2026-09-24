import { voyaCommands } from "@voya/client/transport";
import { isRuntimeTransitioning } from "@voya/client/runtime-action";
import { runtimeActionPending, useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";

/** Re-check immediately before deleting, serialize with connect/switch actions. */
export async function deleteSafely(ids: string[], remove: () => Promise<unknown>) {
  if (runtimeActionPending()) throw new Error("Runtime transition in progress");
  const store = useRuntimeActionStore.getState();
  store.startAction("disconnect");
  try {
    const status = await voyaCommands().runtimeStatus();
    if (isRuntimeTransitioning(status.state)) throw new Error("Runtime transition in progress");
    const groups = await voyaCommands().listPolicyGroups();
    const affected = ids.includes(status.activeProfileId ?? "") || groups.entries.some((entry) => entry.isActive && entry.members.some((member) => ids.includes(member.profileId)));
    if (status.state !== "disconnected" && (affected || status.state === "cleanupPending")) {
      const stopped = await voyaCommands().disconnectCore();
      useRuntimeEventStore.getState().setCoreState(stopped);
      if (stopped.state !== "disconnected") throw new Error("Tunnel has not disconnected");
    }
    await remove();
  } finally { store.finishAction(); }
}
