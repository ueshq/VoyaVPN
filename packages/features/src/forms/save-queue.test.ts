import { QueryClient } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vite-plus/test";

import { saveQueue } from "./save-queue";

describe("saveQueue", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("runs the job behind one that rejects, and still settles", async () => {
    const reported = vi.spyOn(console, "error").mockImplementation(() => {});
    const queue = saveQueue(new QueryClient());
    const ran: string[] = [];

    queue.enqueue({}, async () => {
      ran.push("first");
      throw new Error("the save failed and did not report itself");
    });
    queue.enqueue({}, async () => {
      ran.push("second");
    });
    expect(queue.isSaving()).toBe(true);
    await queue.settled();

    expect(ran).toEqual(["first", "second"]);
    expect(queue.isSaving()).toBe(false);
    expect(reported).toHaveBeenCalledTimes(1);
  });

  it("keeps a replaced field's place and sends only its latest value", async () => {
    const queue = saveQueue(new QueryClient());
    const ran: string[] = [];
    const first = {};
    const second = {};
    let release = () => {};
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });

    // The running job holds the queue while the waiting ones are replaced.
    queue.enqueue({}, () => held);
    queue.enqueue(first, async () => void ran.push("first, stale"));
    queue.enqueue(second, async () => void ran.push("second"));
    queue.enqueue(first, async () => void ran.push("first, latest"));
    await Promise.resolve();
    release();
    await queue.settled();

    expect(ran).toEqual(["first, latest", "second"]);
  });

  it("tells its listeners when saving starts and stops", async () => {
    const queue = saveQueue(new QueryClient());
    const states: boolean[] = [];
    const unsubscribe = queue.subscribe(() => states.push(queue.isSaving()));

    queue.enqueue({}, async () => {});
    await queue.settled();
    unsubscribe();

    expect(states).toEqual([true, false]);
  });
});
