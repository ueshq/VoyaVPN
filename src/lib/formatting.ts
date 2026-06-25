const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"] as const;
const RATE_UNITS = ["KB/s", "MB/s", "GB/s"] as const;

function formatScaledBinary(value: number, units: readonly string[], fixedFraction = false) {
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

  let scaled = value / 1024;
  let unitIndex = 0;

  while (scaled >= 1024 && unitIndex < RATE_UNITS.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }

  return `${scaled >= 10 ? scaled.toFixed(0) : scaled.toFixed(1)} ${RATE_UNITS[unitIndex]}`;
}

export function formatDelay(delay: number | null | undefined, fallback?: string | null) {
  if (typeof delay === "number" && delay > 0) {
    return `${delay} ms`;
  }

  return fallback || "";
}

export function formatSpeed(speed: number | null | undefined) {
  if (!speed || speed <= 0) {
    return "";
  }

  return formatScaledBinary(speed, ["B/s", "KB/s", "MB/s", "GB/s", "TB/s"], true);
}

export function formatTraffic(value: number | null | undefined) {
  if (!value || value <= 0) {
    return "";
  }

  return formatScaledBinary(value, BYTE_UNITS);
}
