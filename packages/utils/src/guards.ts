/** A plain object, such as parsed JSON, as opposed to `null`, an array or a primitive. */
export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
