export type SettingsChange = { path: string; value: unknown };

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function changedFields(before: unknown, after: unknown, path = ""): SettingsChange[] {
  if (JSON.stringify(before) === JSON.stringify(after)) return [];
  if (isObject(before) && isObject(after)) {
    return Object.keys(after).flatMap((key) => changedFields(before[key], after[key], path ? `${path}.${key}` : key));
  }
  return [{ path, value: after }];
}

export function applyChanges<T>(original: T, changes: SettingsChange[]): T {
  const result = structuredClone(original);
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
export type SaveFailure = { message: string; fields: Record<string, string> };
type DraftSnapshot = { changes: SettingsChange[]; failures: Record<string, SaveFailure>; saved: boolean };

/** Page-local rejected edits; only queued writes survive detach(). */
export class SettingsDraft<T> {
  private edits = new Map<string, Edit>();
  private keys = new Map<string, object>();
  private listeners = new Set<() => void>();
  private active = false;
  private revision = 0;
  private snapshot: DraftSnapshot = { changes: [], failures: {}, saved: false };

  constructor(private readonly options: {
    read: () => T | undefined;
    write: (change: SettingsChange) => Promise<T>;
    enqueue: (key: object, job: () => Promise<void>) => void;
    failure: (error: unknown) => SaveFailure;
    report: (failure: SaveFailure) => void;
  }) {}

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };
  getSnapshot = () => this.snapshot;
  attach = () => { this.active = true; };
  detach = () => {
    this.active = false;
    this.edits.clear();
    this.snapshot = { changes: [], failures: {}, saved: false };
  };

  update = (updater: (current: T) => T) => {
    const original = this.options.read();
    if (!original) return;
    const current = applyChanges(original, [...this.edits.values()]);
    for (const change of changedFields(current, updater(current))) {
      const edit = { ...change, revision: ++this.revision };
      if (this.active) this.edits.set(change.path, edit);
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
    if (!key) { key = {}; this.keys.set(edit.path, key); }
    const failures = { ...this.snapshot.failures };
    delete failures[edit.path];
    this.publish(failures, false);
    this.options.enqueue(key, async () => {
      try {
        await this.options.write(edit);
        if (this.edits.get(edit.path)?.revision === edit.revision) this.edits.delete(edit.path);
        this.publish(this.snapshot.failures, true);
      } catch (error) {
        const failure = this.options.failure(error);
        if (!this.active) {
          this.options.report(failure);
        } else if (this.edits.get(edit.path)?.revision === edit.revision) {
          this.publish({ ...this.snapshot.failures, [edit.path]: failure }, false);
        }
      }
    });
  }

  private publish(failures: DraftSnapshot["failures"], saved: boolean) {
    if (!this.active) return;
    this.snapshot = { changes: [...this.edits.values()], failures, saved };
    for (const listener of this.listeners) listener();
  }
}
