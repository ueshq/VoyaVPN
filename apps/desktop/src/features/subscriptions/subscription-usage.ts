import type { SubscriptionMetadata } from "@/ipc/bindings";

const DAY_MS = 24 * 60 * 60 * 1000;

export function metadataBySubscriptionId(
  items: readonly SubscriptionMetadata[],
): Map<string, SubscriptionMetadata> {
  return new Map(items.map((item) => [item.subscriptionId, item]));
}

/** Total reported usage, or `null` when the server reported neither figure. */
function usedTrafficBytes(metadata: SubscriptionMetadata | null | undefined): number | null {
  if (metadata == null || (metadata.uploadBytes == null && metadata.downloadBytes == null)) {
    return null;
  }
  return (metadata.uploadBytes ?? 0) + (metadata.downloadBytes ?? 0);
}

/** Remaining quota clamped to zero, or `null` without a positive total. */
export function remainingTrafficBytes(
  metadata: SubscriptionMetadata | null | undefined,
): number | null {
  const total = metadata?.totalBytes;
  if (metadata == null || total == null || total <= 0) {
    return null;
  }
  return Math.max(0, total - (usedTrafficBytes(metadata) ?? 0));
}

/** Fraction of the quota consumed in [0, 1], or `null` without a total. */
export function usageRatio(metadata: SubscriptionMetadata | null | undefined): number | null {
  const total = metadata?.totalBytes;
  if (metadata == null || total == null || total <= 0) {
    return null;
  }
  return Math.min(1, Math.max(0, (usedTrafficBytes(metadata) ?? 0) / total));
}

/** Whole days until expiry (ceiling), clamped to zero; `null` without expiry. */
export function remainingDays(
  expireAtUnixSeconds: number | null | undefined,
  nowMs: number = Date.now(),
): number | null {
  if (expireAtUnixSeconds == null || expireAtUnixSeconds <= 0) {
    return null;
  }
  return Math.max(0, Math.ceil((expireAtUnixSeconds * 1000 - nowMs) / DAY_MS));
}

export function isExpired(
  expireAtUnixSeconds: number | null | undefined,
  nowMs: number = Date.now(),
): boolean {
  return expireAtUnixSeconds != null && expireAtUnixSeconds > 0 && expireAtUnixSeconds * 1000 <= nowMs;
}

export function isTrafficExhausted(metadata: SubscriptionMetadata | null | undefined): boolean {
  return remainingTrafficBytes(metadata) === 0;
}

/** Largest sensible unit + signed value for `Intl.RelativeTimeFormat`. */
export function relativeTimeFrom(
  unixSeconds: number,
  nowMs: number = Date.now(),
): { unit: Intl.RelativeTimeFormatUnit; value: number } {
  const deltaSeconds = Math.round(unixSeconds - nowMs / 1000);
  const magnitude = Math.abs(deltaSeconds);
  if (magnitude < 60) {
    return { unit: "second", value: deltaSeconds };
  }
  if (magnitude < 3600) {
    return { unit: "minute", value: Math.trunc(deltaSeconds / 60) };
  }
  if (magnitude < 86_400) {
    return { unit: "hour", value: Math.trunc(deltaSeconds / 3600) };
  }
  return { unit: "day", value: Math.trunc(deltaSeconds / 86_400) };
}
