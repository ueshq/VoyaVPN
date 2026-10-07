import { beforeEach, expect, it } from "vitest";
import { installTestStorage } from "./test-storage";
import { useConnectionPreferences } from "./connection-preferences";

const storage = installTestStorage();
const key = "voyavpn.mobile.connection";
beforeEach(() => { storage.clear(); useConnectionPreferences.setState({ autoConnect: false, recentIds: [] }); });
it("persists opt-in and bounds recent successful selections without duplicate ids", async () => {
  const state = useConnectionPreferences.getState();
  state.setAutoConnect(true);
  for (const id of ["a", "b", "c", "d", "e", "f", "c"]) state.rememberConnected(id);
  expect(useConnectionPreferences.getState().recentIds).toEqual(["c", "f", "e", "d", "b"]);
  await useConnectionPreferences.persist.rehydrate();
  expect(useConnectionPreferences.getState().autoConnect).toBe(true);
});
it("does not turn malformed or old preferences into permission to auto-connect", async () => {
  for (const state of [{}, { autoConnect: "true", recentIds: "node" }, { autoConnect: null, recentIds: [null, 2, "", "a", "a", "b"] }]) {
    storage.write(key, JSON.stringify({ state, version: 0 }));
    await useConnectionPreferences.persist.rehydrate();
    expect(useConnectionPreferences.getState().autoConnect).toBe(false);
  }
  expect(useConnectionPreferences.getState().recentIds).toEqual(["a", "b"]);
});
