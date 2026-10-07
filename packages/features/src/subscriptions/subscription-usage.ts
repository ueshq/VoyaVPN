import type { SubscriptionMetadata } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n/core";
import { formatBytes } from "@voya/utils/formatting";

const DAY_MS = 24 * 60 * 60 * 1000;

export function metadataBySubscriptionId(items: readonly SubscriptionMetadata[]): Map<string, SubscriptionMetadata> {
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
export function remainingTrafficBytes(metadata: SubscriptionMetadata | null | undefined): number | null {
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

export function isExpired(expireAtUnixSeconds: number | null | undefined, nowMs: number = Date.now()): boolean {
  return expireAtUnixSeconds != null && expireAtUnixSeconds > 0 && expireAtUnixSeconds * 1000 <= nowMs;
}

export function isTrafficExhausted(metadata: SubscriptionMetadata | null | undefined): boolean {
  return remainingTrafficBytes(metadata) === 0;
}

/**
 * What a subscription's server says is left of it, in words: the remaining
 * traffic when it reports a quota, and the remaining days — or that it has
 * expired — when it reports an expiry. A figure the server does not report is
 * left out rather than shown as zero, and one that has run out is marked.
 */
export function subscriptionUsageStats(
  metadata: SubscriptionMetadata | null | undefined,
  t: TranslationFunction,
  nowMs: number = Date.now(),
): { destructive: boolean; key: "days" | "traffic"; label: string }[] {
  const stats: ReturnType<typeof subscriptionUsageStats> = [];
  const remaining = remainingTrafficBytes(metadata);
  if (remaining != null) {
    stats.push({
      destructive: remaining === 0,
      key: "traffic",
      label: t("home.subscriptionCard.remainingTraffic", { amount: formatBytes(remaining) }),
    });
  }
  const days = remainingDays(metadata?.expireAt, nowMs);
  if (days != null) {
    const expired = isExpired(metadata?.expireAt, nowMs);
    stats.push({
      destructive: expired,
      key: "days",
      label: expired ? t("home.subscriptionCard.expired") : t("home.subscriptionCard.remainingDays", { days }),
    });
  }

  return stats;
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
