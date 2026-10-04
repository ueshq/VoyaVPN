import { describe, expect, it } from "vitest";

import { download } from "./download.mjs";

describe("seed download", () => {
  it("returns the body of a successful response", async () => {
    const fetchImpl = async () => new Response("payload");

    expect((await download("https://example.test/a", {}, { fetchImpl })).toString()).toBe("payload");
  });

  it("names the URL of a response that is not a success", async () => {
    const fetchImpl = async () => new Response("", { status: 404, statusText: "Not Found" });

    await expect(download("https://example.test/a", {}, { fetchImpl })).rejects.toThrow(
      "download failed 404 Not Found: https://example.test/a",
    );
  });

  it("gives up on a connection that stalls instead of waiting forever", async () => {
    // Never answers on its own; only the abort signal ends it.
    const fetchImpl = (_url, { signal }) =>
      new Promise((_resolve, reject) => {
        signal.addEventListener("abort", () => reject(signal.reason));
      });

    await expect(download("https://example.test/a", {}, { fetchImpl, timeoutMs: 20 })).rejects.toThrow(
      "download timed out after 0s: https://example.test/a",
    );
  });
});
