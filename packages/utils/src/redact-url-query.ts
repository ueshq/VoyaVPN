/**
 * Masks a URL's query values for display.
 *
 * Subscription links carry the account token in their query (`?token=…`), so
 * a preview or a list must not print the URL verbatim — someone glancing at
 * the screen should not leave with credentials. The scheme, host and path
 * stay: they are what identifies the source. Values the URL cannot live
 * without reading later (the editor keeps the full URL) are unaffected; this
 * is a display helper only.
 */
export function redactUrlQuery(url: string): string {
  const queryStart = url.indexOf("?");
  if (queryStart === -1) return url;

  const base = url.slice(0, queryStart);
  const query = url.slice(queryStart + 1);
  const fragmentStart = query.indexOf("#");
  const fragment = fragmentStart === -1 ? "" : query.slice(fragmentStart);
  const pairs = (fragmentStart === -1 ? query : query.slice(0, fragmentStart)).split("&").filter(Boolean);

  return `${base}?${pairs.map((pair) => `${pair.split("=")[0]}=…`).join("&")}${fragment}`;
}

const URL_HOST = /^[a-z][a-z\d+.-]*:\/\/(?:[^/?#@]*@)?(\[[^\]]*\]|[^:/?#]*)/i;

/**
 * The host of a URL, for display beside its redacted form; empty when the
 * text is not a URL.
 *
 * Read with a pattern rather than `new URL`: React Native's `URL` is not the
 * WHATWG one — it throws on input the real one accepts and accepts input the
 * real one rejects — and this runs while rendering, where a throw takes the
 * screen down.
 */
export function urlHost(url: string): string {
  return URL_HOST.exec(url.trim())?.[1] ?? "";
}
