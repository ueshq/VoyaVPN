export type SettingsChange = { path: string; value: unknown };

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function changedFields(
  before: unknown,
  after: unknown,
  path = "",
): SettingsChange[] {
  if (JSON.stringify(before) === JSON.stringify(after)) return [];
  if (isObject(before) && isObject(after)) {
    return Object.keys(after).flatMap((key) =>
      changedFields(before[key], after[key], path ? `${path}.${key}` : key),
    );
  }
  return [{ path, value: after }];
}

/**
 * A deep copy of a value that came from, or is headed for, the IPC boundary.
 *
 * Not `structuredClone`: React Native does not have it. RN 0.87 ships an
 * implementation but keeps it private to its own `Performance` web API and
 * never installs the global, so calling it on Hermes throws
 * `ReferenceError: Property 'structuredClone' doesn't exist` — which on a
 * release build is fatal, and took the whole app down when the settings screen
 * mounted. Everything this module clones is an IPC DTO, which is by definition
 * what `JSON.parse` produced, so a round trip through JSON is not a
 * compromise here: it is the same value. `changedFields` above already
 * compares by `JSON.stringify` for the same reason.
 */
export function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

export function applyChanges<T>(original: T, changes: SettingsChange[]): T {
  const result = cloneJson(original);
  for (const { path, value } of changes) {
    const keys = path.split(".");
    let target: unknown = result;
    for (const key of keys.slice(0, -1)) {
      if (!isObject(target)) break;
      target = target[key];
    }
    const key = keys.at(-1);
    if (isObject(target) && key) target[key] = value;
  }
  return result;
}

type Edit = SettingsChange & { revision: number };
type Writer<T> = (change: SettingsChange) => Promise<T>;
export type SaveFailure = { message: string; fields: Record<string, string> };
type DraftSnapshot = {
  changes: SettingsChange[];
  failures: Record<string, SaveFailure>;
  saved: boolean;
};

/** Session drafts survive navigation; a rejected revision remains available to retry. */
export class SettingsDraft<T> {
  private edits = new Map<string, Edit>();
  private keys = new Map<string, object>();
  private listeners = new Set<() => void>();
  private active = false;
  private revision = 0;
  private snapshot: DraftSnapshot = { changes: [], failures: {}, saved: false };
  private write: Writer<T>;

  constructor(
    private readonly options: {
      read: () => T | undefined;
      write: Writer<T>;
      enqueue: (key: object, job: () => Promise<void>) => void;
      failure: (error: unknown) => SaveFailure;
      report: (failure: SaveFailure) => void;
    },
  ) {
    this.write = options.write;
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  getSnapshot = () => this.snapshot;
  /**
   * A pane is showing the draft. A draft outlives its pane and is reused by
   * every later mount, so the pane's writer replaces the one it had: saves,
   * including ones still queued, go through the newest pane's closure.
   */
  attach = (write: Writer<T>) => {
    this.active = true;
    this.write = write;
  };
  detach = () => {
    this.active = false;
  };

  update = (updater: (current: T) => T) => {
    const original = this.options.read();
    if (!original) return;
    const current = applyChanges(original, [...this.edits.values()]);
    for (const change of changedFields(current, updater(current))) {
      const edit = { ...change, revision: ++this.revision };
      this.edits.set(change.path, edit);
      this.submit(edit);
    }
  };

  retry = () => {
    for (const path of Object.keys(this.snapshot.failures)) {
      const edit = this.edits.get(path);
      if (edit) this.submit(edit);
    }
  };

  private submit(edit: Edit) {
    let key = this.keys.get(edit.path);
    if (!key) {
      key = {};
      this.keys.set(edit.path, key);
    }
    const failures = { ...this.snapshot.failures };
    delete failures[edit.path];
    this.publish(failures, false);
    this.options.enqueue(key, async () => {
      try {
        await this.write(edit);
        if (this.edits.get(edit.path)?.revision === edit.revision)
          this.edits.delete(edit.path);
        this.publish(this.snapshot.failures, true);
      } catch (error) {
        const failure = this.options.failure(error);
        if (!this.active) {
          this.options.report(failure);
        }
        if (this.edits.get(edit.path)?.revision === edit.revision) {
          this.publish(
            { ...this.snapshot.failures, [edit.path]: failure },
            false,
          );
        }
      }
    });
  }

  private publish(failures: DraftSnapshot["failures"], saved: boolean) {
    this.snapshot = { changes: [...this.edits.values()], failures, saved };
    for (const listener of this.listeners) listener();
  }
}
