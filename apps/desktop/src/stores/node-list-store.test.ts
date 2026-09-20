import { beforeEach, describe, expect, it } from "vitest";

import { useNodeListStore } from "./node-list-store";

const STORAGE_KEY = "voyavpn.nodeList";

function storedView() {
  return (JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? "{}") as { state?: unknown }).state;
}

async function rehydrateFrom(state: unknown) {
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify({ state, version: 0 }));
  await useNodeListStore.persist.rehydrate();
}

describe("node list store", () => {
  beforeEach(() => {
    window.localStorage.clear();
    useNodeListStore.setState(useNodeListStore.getInitialState());
  });

  it("collapses and expands a group by toggling it", () => {
    const { toggleGroup } = useNodeListStore.getState();

    toggleGroup("subscription:a");
    toggleGroup("subscription:b");
    expect(useNodeListStore.getState().collapsedGroups).toEqual(["subscription:a", "subscription:b"]);

    toggleGroup("subscription:a");
    expect(useNodeListStore.getState().collapsedGroups).toEqual(["subscription:b"]);
  });

  it("remembers only the view between launches", () => {
    useNodeListStore.getState().setHideUnreachable(true);
    useNodeListStore.getState().setSortByLatency(true);
    useNodeListStore.getState().toggleGroup("local");

    expect(storedView()).toEqual({
      collapsedGroups: ["local"],
      hideUnreachable: true,
      sortByLatency: true,
    });
  });

  it("restores a stored view", async () => {
    await rehydrateFrom({ collapsedGroups: ["local"], hideUnreachable: true, sortByLatency: true });

    expect(useNodeListStore.getState()).toMatchObject({
      collapsedGroups: ["local"],
      hideUnreachable: true,
      sortByLatency: true,
    });
  });

  it("keeps the valid parts of a view another build wrote", async () => {
    await rehydrateFrom({
      collapsedGroups: ["local", 7, null, "subscription:a"],
      hideUnreachable: "yes",
      sortByLatency: true,
    });

    expect(useNodeListStore.getState()).toMatchObject({
      collapsedGroups: ["local", "subscription:a"],
      hideUnreachable: false,
      sortByLatency: true,
    });

    await rehydrateFrom({ collapsedGroups: "local", sortByLatency: 1 });
    expect(useNodeListStore.getState()).toMatchObject({
      collapsedGroups: ["local", "subscription:a"],
      hideUnreachable: false,
      sortByLatency: true,
    });
  });

  it("ignores a stored view that is not an object", async () => {
    useNodeListStore.getState().setSortByLatency(true);

    await rehydrateFrom(["collapsedGroups"]);

    expect(useNodeListStore.getState()).toMatchObject({ collapsedGroups: [], sortByLatency: true });
  });
});
