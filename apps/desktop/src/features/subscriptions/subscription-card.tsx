import type { SubscriptionMetadata } from "@/ipc/bindings";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatBytes } from "@voya/utils/formatting";
import { cn } from "@voya/ui/lib/utils";
import {
  isExpired,
  isTrafficExhausted,
  usageRatio,
  relativeTimeFrom,
  remainingDays,
  remainingTrafficBytes,
} from "@voya/features/subscriptions/subscription-usage";

type TranslateFn = ReturnType<typeof useI18n>["t"];

/** Compact "remaining traffic · remaining days · last updated" stat row. */
export function SubscriptionMetaLine({
  className,
  language,
  metadata,
  t,
}: {
  className?: string;
  language: string;
  metadata: SubscriptionMetadata | null | undefined;
  t: TranslateFn;
}) {
  const stats: { destructive?: boolean; key: string; label: string }[] = [];
  const remaining = remainingTrafficBytes(metadata);
  if (remaining != null) {
    stats.push({
      destructive: remaining === 0,
      key: "traffic",
      label: t("home.subscriptionCard.remainingTraffic", {
        amount: formatBytes(remaining),
      }),
    });
  }
  const days = remainingDays(metadata?.expireAt);
  if (days != null) {
    stats.push(
      isExpired(metadata?.expireAt)
        ? {
            destructive: true,
            key: "days",
            label: t("home.subscriptionCard.expired"),
          }
        : {
            key: "days",
            label: t("home.subscriptionCard.remainingDays", { days }),
          },
    );
  }
  if (metadata?.lastUpdateAt != null) {
    const { unit, value } = relativeTimeFrom(metadata.lastUpdateAt);
    const formatted = new Intl.RelativeTimeFormat(language, {
      numeric: "auto",
    }).format(value, unit);
    stats.push({
      key: "updated",
      label: t("home.subscriptionCard.lastUpdated", { time: formatted }),
    });
  }
  if (stats.length === 0) {
    return null;
  }

  const ratio = usageRatio(metadata);
  return (
    <div className={cn("grid gap-2", className)}>
      <p
        className={cn(
          "flex flex-wrap items-center gap-x-2 text-xs text-muted-foreground",
        )}
      >
        {stats.map((stat, index) => (
          <span className="inline-flex items-center gap-x-2" key={stat.key}>
            {index > 0 ? <span aria-hidden="true">·</span> : null}
            <span
              className={
                stat.destructive ? "font-medium text-danger" : undefined
              }
            >
              {stat.label}
            </span>
          </span>
        ))}
      </p>
      {ratio !== null ? (
        <div
          aria-hidden="true"
          className="h-1 overflow-hidden rounded-full bg-muted"
        >
          <div
            className={cn(
              "h-full rounded-full bg-brand",
              isTrafficExhausted(metadata) && "bg-destructive",
            )}
            style={{ width: `${Math.round(ratio * 100)}%` }}
          />
        </div>
      ) : null}
    </div>
  );
}
