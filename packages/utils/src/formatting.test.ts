import { describe, expect, it } from "vitest";

import { formatBytes, formatBytesPerSecond, formatDelay } from "./formatting";

describe("formatting", () => {
  it("formats byte counts with shared binary units", () => {
    expect(formatBytes(undefined)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.0 GB");
  });

  it("formats live byte rates with the sidebar speed-row precision", () => {
    expect(formatBytesPerSecond(0)).toBe("0 B/s");
    expect(formatBytesPerSecond(512.6)).toBe("513 B/s");
    expect(formatBytesPerSecond(1024)).toBe("1.0 KB/s");
    expect(formatBytesPerSecond(2048)).toBe("2.0 KB/s");
    expect(formatBytesPerSecond(10 * 1024)).toBe("10 KB/s");
    expect(formatBytesPerSecond(1024 ** 2)).toBe("1.0 MB/s");
    expect(formatBytesPerSecond(1024 ** 3)).toBe("1.0 GB/s");
    expect(formatBytesPerSecond(1024 ** 4)).toBe("1024 GB/s");
  });

  it("hides absent or unsuccessful latency measurements", () => {
    expect(formatDelay(42)).toBe("42 ms");
    expect(formatDelay(0)).toBe("");
    expect(formatDelay(null)).toBe("");
    expect(formatDelay(undefined)).toBe("");
    expect(formatDelay(-1)).toBe("");
  });

});
