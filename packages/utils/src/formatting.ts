const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"] as const;
const RATE_UNITS = ["B/s", "KB/s", "MB/s", "GB/s"] as const;

function formatScaledBinary(value: number, units: readonly string[], fixedFraction: boolean) {
  let scaled = value;
  let unitIndex = 0;

  while (scaled >= 1024 && unitIndex < units.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }

  if (unitIndex === 0) {
    return `${scaled.toFixed(0)} ${units[unitIndex]}`;
  }

  return `${fixedFraction || scaled < 10 ? scaled.toFixed(1) : scaled.toFixed(0)} ${units[unitIndex]}`;
}

export function formatBytes(value: number | null | undefined) {
  return formatScaledBinary(value ?? 0, BYTE_UNITS, true);
}

export function formatBytesPerSecond(value: number) {
  if (value < 1024) {
    return `${Math.round(value)} B/s`;
  }

  return formatScaledBinary(value, RATE_UNITS, false);
}

/** Zero-padded `HH:MM:SS`; the hours do not wrap, so a long duration keeps counting. */
export function formatClock(hours: number, minutes: number, seconds: number) {
  return [hours, minutes, seconds].map((value) => String(value).padStart(2, "0")).join(":");
}

/** A measured latency, or an empty string when no measurement is available. */
export function formatDelay(delay: number | null | undefined) {
  if (typeof delay === "number" && delay > 0) {
    return `${delay} ms`;
  }

  return "";
}
