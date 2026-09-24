import type { MockBackend } from "@voya/client/mock-backend";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { deleteSafely } from "./delete-safely";

beforeEach(() => {
  registerMobileBackend();
  useRuntimeActionStore.setState(useRuntimeActionStore.getInitialState());
});

test("disconnect failure retains the selected node and releases the action lock", async () => {
  const backend = voyaTransport() as MockBackend;
  jest.spyOn(backend.commands, "runtimeStatus").mockResolvedValue({ ...await backend.commands.runtimeStatus(), state: "connected", activeProfileId: "selected" });
  jest.spyOn(backend.commands, "disconnectCore").mockRejectedValue(new Error("stop failed"));
  const remove = jest.fn();
  await expect(deleteSafely(["selected"], remove)).rejects.toThrow("stop failed");
  expect(remove).not.toHaveBeenCalled();
  expect(useRuntimeActionStore.getState().pendingAction).toBeNull();
});

test("claims the lock before asynchronous inspection so a second delete cannot race", async () => {
  const backend = voyaTransport() as MockBackend;
  const status = await backend.commands.runtimeStatus();
  let resolveStatus!: (value: typeof status) => void;
  jest.spyOn(backend.commands, "runtimeStatus").mockImplementation(() => new Promise((resolve) => { resolveStatus = resolve; }));
  const remove = jest.fn();
  const first = deleteSafely(["unselected"], remove);
  await expect(deleteSafely(["unselected"], remove)).rejects.toThrow("Runtime transition");
  resolveStatus(status);
  await first;
  expect(remove).toHaveBeenCalledTimes(1);
  expect(backend.state.calls.some((call) => call.command === "disconnectCore")).toBe(false);
});
