/** A text field's value, trimmed; blank input becomes `null`, which the contract reads as "not set". */
export function trimToNull(value: string | null | undefined): string | null {
  return value?.trim() || null;
}

/** Entries typed one per line or separated by commas, trimmed, with blanks dropped. */
export function splitList(value: string | null | undefined): string[] {
  return (value ?? "")
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter(Boolean);
}
