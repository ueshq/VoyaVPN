import type { QueryClient } from "@tanstack/react-query";

/** One writer for app settings and DNS, including work that outlives the page. */
class SettingsSaveQueue {
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

const queues = new WeakMap<QueryClient, SettingsSaveQueue>();

export function settingsSaveQueue(client: QueryClient): SettingsSaveQueue {
  let queue = queues.get(client);
  if (!queue) {
    queue = new SettingsSaveQueue();
    queues.set(client, queue);
  }
  return queue;
}
