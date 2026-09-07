import { useEffect, useId, useRef, useSyncExternalStore } from "react";

/**
 * Registry for settings panes that keep their own draft instead of joining the
 * app-settings controller (today: the DNS pane, which saves through dedicated
 * commands with server-side validation).
 *
 * The Settings surface renders exactly one screen, so leaving the shell tab
 * unmounts such a pane. Without this registry its unsaved edits are dropped
 * silently: the footer's "Unsaved changes"/"Save all" and the navigation leave
 * guard only know about the app-settings draft. Panes register their dirty
 * state plus a save/discard pair here so the surface can speak for all drafts.
 */
type SettingsDirtySource = {
  /** Whether the pane currently holds unsaved edits. */
  dirty: boolean;
  /** Drops the draft and restores the authoritative values. */
  discard: () => Promise<void> | void;
  /** Persists the draft; resolves to an error message, or null on success. */
  save: () => Promise<string | null>;
};

type SourceRef = { current: SettingsDirtySource };

const sources = new Map<string, SourceRef>();
const listeners = new Set<() => void>();
let anyDirty = false;

function publish() {
  anyDirty = [...sources.values()].some((source) => source.current.dirty);
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot() {
  return anyDirty;
}

/**
 * Publishes a pane's draft state to the Settings surface for as long as the
 * pane is mounted. The latest callbacks are re-published on every render, so
 * the surface never saves through a stale closure.
 */
export function useRegisterSettingsDirtySource(source: SettingsDirtySource): void {
  const id = useId();
  const latest = useRef(source);

  useEffect(() => {
    sources.set(id, latest);
    return () => {
      sources.delete(id);
      publish();
    };
  }, [id]);

  useEffect(() => {
    latest.current = source;
    publish();
  });
}

/** Whether any registered pane holds unsaved edits. */
export function useSettingsDirtySources(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

/**
 * Saves every dirty pane, stopping at the first failure and resolving to its
 * message so the caller can show it outside the pane (the leave-guard dialog
 * covers the pane's own error line).
 */
export async function saveSettingsDirtySources(): Promise<string | null> {
  for (const source of [...sources.values()]) {
    if (!source.current.dirty) {
      continue;
    }
    const error = await source.current.save();
    if (error !== null) {
      return error;
    }
  }
  return null;
}

/** Discards every dirty pane draft. */
export async function discardSettingsDirtySources(): Promise<void> {
  for (const source of [...sources.values()]) {
    if (source.current.dirty) {
      await source.current.discard();
    }
  }
}
