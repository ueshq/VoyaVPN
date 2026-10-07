import { describe, expect, it } from "vite-plus/test";

import { formatDateTime } from "./format-date-time";

describe("formatDateTime", () => {
  it("writes a moment the way the language does, from a timestamp or its text", () => {
    const moment = Date.UTC(2026, 9, 4, 12, 30, 5);

    expect(formatDateTime(moment, "en")).toBe(new Date(moment).toLocaleString("en"));
    expect(formatDateTime(new Date(moment).toISOString(), "zh-Hans")).toBe(
      new Date(moment).toLocaleString("zh-Hans"),
    );
  });

  it("hands back text that is not a date instead of printing Invalid Date", () => {
    expect(formatDateTime("just now", "en")).toBe("just now");
  });
});
