/**
 * The message an error carries. With a `fallback`, an error without usable
 * text (an empty message, or a value that is neither an `Error` nor a string)
 * reads as the fallback instead.
 */
export function getErrorMessage(error: unknown, fallback?: string): string {
  if (fallback === undefined) {
    return error instanceof Error ? error.message : String(error);
  }

  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === "string" && error) {
    return error;
  }

  return fallback;
}
