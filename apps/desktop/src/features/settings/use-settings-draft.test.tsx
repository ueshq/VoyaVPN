import { act } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { saveQueue } from "@voya/features/forms/save-queue";
import { createTestQueryClient, renderHookWithQuery } from "@/test/render";

import type { SettingsChange } from "./settings-draft";
import { useSettingsDraft } from "./use-settings-draft";

type Draftable = { level: string };

const KEY = ["draft-under-test"];

function writer() {
  return vi.fn(async (change: SettingsChange) => ({ level: String(change.value) }));
}

function setup() {
  const client = createTestQueryClient();
  client.setQueryData<Draftable>(KEY, { level: "info" });
  let write = writer();
  const mount = () =>
    renderHookWithQuery(
      () =>
        useSettingsDraft<Draftable>({
          data: client.getQueryData<Draftable>(KEY),
          queryKey: KEY,
          write,
        }),
      { queryClient: client },
    );
  return {
    mount,
    setWrite: (next: ReturnType<typeof writer>) => {
      write = next;
    },
    settle: () => act(() => saveQueue(client).settled()),
  };
}

describe("useSettingsDraft", () => {
  it("saves through the newest mount's writer after the pane remounts", async () => {
    const { mount, setWrite, settle } = setup();
    const first = writer();
    const second = writer();
    setWrite(first);
    mount().unmount();

    // The draft survives the unmount; the next mount brings its own writer.
    setWrite(second);
    const { result } = mount();
    act(() => result.current.update((current) => ({ ...current, level: "debug" })));
    await settle();

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith(expect.objectContaining({ path: "level", value: "debug" }));
    expect(result.current.saved).toBe(true);
  });

  it("sends a queued save through the writer of the latest render", async () => {
    const { mount, setWrite, settle } = setup();
    const first = writer();
    const second = writer();
    setWrite(first);
    const { rerender, result } = mount();

    act(() => {
      result.current.update((current) => ({ ...current, level: "warn" }));
      setWrite(second);
      rerender();
    });
    await settle();

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith(expect.objectContaining({ path: "level", value: "warn" }));
  });
});
