import { isRecord } from "@voya/utils/guards";

/**
 * A `persist` merge for stored state that an older or newer build may have
 * written: `read` returns only the stored fields that are still valid, and every
 * other field keeps its current value.
 */
export function mergeValidated<State>(read: (stored: Record<string, unknown>) => Partial<State>) {
  return (persisted: unknown, current: State): State =>
    isRecord(persisted) ? { ...current, ...read(persisted) } : current;
}
