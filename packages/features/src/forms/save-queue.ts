import type { QueryClient } from "@tanstack/react-query";

/**
 * One serial writer per `QueryClient` for settings-class saves — work that
 * must not interleave and sometimes outlives the page it started on. The
 * updates flow only waits for it to drain; settings and DNS enqueue into it.
 */
class SaveQueue {
  private jobs = new Map<object, () => Promise<void>>();
  private running = false;
  private listeners = new Set<() => void>();
  private waiters = new Set<() => void>();

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };

  isSaving = () => this.running;

  enqueue(key: object, job: () => Promise<void>) {
    // Replacing a waiting field retains its position and only sends its latest value.
    this.jobs.set(key, job);
    if (this.running) return;
    this.running = true;
    this.publish();
    queueMicrotask(() => { void this.drain(); });
  }

  settled = (): Promise<void> => this.running
    ? new Promise((resolve) => { this.waiters.add(resolve); })
    : Promise.resolve();

  private async drain() {
    try {
      while (this.jobs.size) {
        const next = this.jobs.entries().next().value;
        if (!next) break;
        const [key, job] = next;
        this.jobs.delete(key);
        // Jobs own their error reporting; failures never poison the writer.
        await job();
      }
    } finally {
      this.running = false;
      this.publish();
      for (const resolve of this.waiters) resolve();
      this.waiters.clear();
    }
  }

  private publish() {
    for (const listener of this.listeners) listener();
  }
}

const queues = new WeakMap<QueryClient, SaveQueue>();

/** The serial save queue owned by this `QueryClient`. */
export function saveQueue(client: QueryClient): SaveQueue {
  let queue = queues.get(client);
  if (!queue) {
    queue = new SaveQueue();
    queues.set(client, queue);
  }
  return queue;
}
