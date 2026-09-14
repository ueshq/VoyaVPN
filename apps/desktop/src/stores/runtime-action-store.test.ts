import { beforeEach, describe, expect, it } from "vitest";

import { runtimeActionPending, useRuntimeActionStore } from "./runtime-action-store";

describe("runtime action store", () => {
  beforeEach(() => {
    useRuntimeActionStore.setState({
      lastError: null,
      modePending: false,
      pendingAction: null,
      switchingId: null,
    });
  });

  it("holds the shared guard for every kind of runtime change", () => {
    const store = useRuntimeActionStore.getState();

    store.failInline("connect", "offline");
    store.startAction("restart");
    expect(useRuntimeActionStore.getState()).toMatchObject({ lastError: null, pendingAction: "restart" });
    expect(runtimeActionPending()).toBe(true);
    store.finishAction();

    store.startSwitch("group:work");
    expect(runtimeActionPending()).toBe(true);
    store.finishSwitch();

    store.setModePending(true);
    expect(runtimeActionPending()).toBe(true);
    store.setModePending(false);

    store.failInline("disconnect", "still running");
    expect(useRuntimeActionStore.getState()).toMatchObject({
      lastError: { action: "disconnect", message: "still running" },
      modePending: false,
      pendingAction: null,
      switchingId: null,
    });
    expect(runtimeActionPending()).toBe(false);
  });
});
