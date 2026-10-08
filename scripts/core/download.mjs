/**
 * How long one download may take, headers to last byte. The sing-box archive
 * is a few tens of megabytes; this is long enough for a slow link and short
 * enough that a stalled connection fails the install step — whose failure
 * `postinstall` reports and carries on from — instead of hanging `vp install`.
 */
const DOWNLOAD_TIMEOUT_MS = 5 * 60 * 1000;

/**
 * One HTTP download for the seed installers. Both fetch a pinned upstream file
 * into memory and verify its digest before anything is written or run, so the
 * response handling has to be the same in both.
 */
export async function download(url, headers, { fetchImpl = fetch, timeoutMs = DOWNLOAD_TIMEOUT_MS } = {}) {
  try {
    const response = await fetchImpl(url, {
      headers,
      redirect: "follow",
      signal: AbortSignal.timeout(timeoutMs),
    });
    if (!response.ok) {
      throw new Error(`download failed ${response.status} ${response.statusText}: ${url}`);
    }
    return Buffer.from(await response.arrayBuffer());
  } catch (error) {
    // The signal covers the body as well; either way the abort surfaces as a
    // bare "The operation was aborted", which says nothing about what was.
    if (error?.name === "TimeoutError") {
      throw new Error(`download timed out after ${Math.round(timeoutMs / 1000)}s: ${url}`, { cause: error });
    }
    throw error;
  }
}
