/**
 * A moment — a millisecond timestamp, or the text of one — as the user's
 * language writes a date and time. Text that is not a date comes back as it
 * is, so a value written some other way still shows.
 *
 * In a module of its own: `formatting` is loaded with the desktop shell, and
 * only screens that open later show a date.
 */
export function formatDateTime(value: number | string, language: string) {
  const date = new Date(value);
  return Number.isFinite(date.getTime()) ? date.toLocaleString(language) : String(value);
}
