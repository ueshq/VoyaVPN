import { describe, expect, it } from "vitest";

import { formatBytes, formatBytesPerSecond, formatDelay, formatSpeed, formatTraffic } from "@/lib/formatting";

describe("formatting", () => {
  it("formats byte counts with shared binary units", () => {
    expect(formatBytes(undefined)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.0 GB");
  });

  it("formats live byte rates with the status bar precision", () => {
    expect(formatBytesPerSecond(512)).toBe("512 B/s");
    expect(formatBytesPerSecond(2048)).toBe("2.0 KB/s");
    expect(formatBytesPerSecond(10 * 1024)).toBe("10 KB/s");
  });

  it("formats speedtest speed values while hiding empty values", () => {
    expect(formatSpeed(null)).toBe("");
    expect(formatSpeed(0)).toBe("");
    expect(formatSpeed(768)).toBe("768 B/s");
    expect(formatSpeed(2048)).toBe("2.0 KB/s");
    expect(formatSpeed(10 * 1024)).toBe("10.0 KB/s");
  });

  it("formats delays with an optional fallback", () => {
    expect(formatDelay(42)).toBe("42 ms");
    expect(formatDelay(0)).toBe("");
    expect(formatDelay(null, "Timeout")).toBe("Timeout");
  });

  it("formats traffic while hiding empty values", () => {
    expect(formatTraffic(undefined)).toBe("");
    expect(formatTraffic(0)).toBe("");
    expect(formatTraffic(1536)).toBe("1.5 KB");
    expect(formatTraffic(10 * 1024)).toBe("10 KB");
  });
});
