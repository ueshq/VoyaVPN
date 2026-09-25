/**
 * One HTTP download for the seed installers. Both fetch a pinned upstream file
 * into memory and verify its digest before anything is written or run, so the
 * response handling has to be the same in both.
 */
export async function download(url, headers, { fetchImpl = fetch } = {}) {
  const response = await fetchImpl(url, { headers, redirect: "follow" });
  if (!response.ok) {
    throw new Error(`download failed ${response.status} ${response.statusText}: ${url}`);
  }
  return Buffer.from(await response.arrayBuffer());
}
