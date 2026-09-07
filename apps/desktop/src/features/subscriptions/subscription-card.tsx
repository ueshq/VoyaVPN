import { useMemo, useState } from "react";
import { LoaderCircle, Plus, RefreshCw, Rss } from "lucide-react";
import { useQuery } from "@tanstack/react-query";

import { Button } from "@voya/ui/components/button";
import { Card, CardContent } from "@voya/ui/components/card";
import { listSubscriptionMetadata, listSubscriptions, updateSubscriptions } from "@/ipc";
import type { Subscription, SubscriptionMetadata } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { useToastStore } from "@/stores/toast-store";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatBytes } from "@voya/utils/formatting";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { cn } from "@voya/ui/lib/utils";

import {
  isSubscriptionUpdateFailure,
  subscriptionUpdateMessages,
} from "./subscription-update-result";
import {
  isExpired,
  isTrafficExhausted,
  metadataBySubscriptionId,
  relativeTimeFrom,
  remainingDays,
  remainingTrafficBytes,
  usageRatio,
} from "./subscription-usage";

type SubscriptionCardProps = {
  /** Subscription owning the active profile, used to pick the shown source. */
  activeSubscriptionId?: string | null;
  onAddSubscription: () => void;
};

/**
 * Hiddify-style subscription profile card: name, remaining traffic and days,
 * last update time, and a one-tap update action. Stats the server never
 * reported are hidden rather than rendered as placeholders.
 */
export function SubscriptionCard({ activeSubscriptionId, onAddSubscription }: SubscriptionCardProps) {
  const { language, t } = useI18n();
  const pushToast = useToastStore((state) => state.pushToast);
  const [updating, setUpdating] = useState(false);
  const subscriptionsQuery = useQuery({
    queryFn: listSubscriptions,
    queryKey: queryKeys.subscriptions,
  });
  const metadataQuery = useQuery({
    queryFn: listSubscriptionMetadata,
    queryKey: queryKeys.subscriptionMetadata,
  });

  const subscriptionsData = subscriptionsQuery.data;
  const subscription = useMemo(
    () => pickDisplayedSubscription(subscriptionsData ?? [], activeSubscriptionId ?? null),
    [activeSubscriptionId, subscriptionsData],
  );
  const metadata = useMemo(() => {
    if (!subscription) {
      return null;
    }
    return metadataBySubscriptionId(metadataQuery.data ?? []).get(subscription.id) ?? null;
  }, [metadataQuery.data, subscription]);

  if (subscriptionsQuery.isLoading) {
    return null;
  }

  if (!subscription) {
    return (
      <Card data-testid="home-subscription-card" className="rounded-2xl bg-surface-raised shadow-raised">
        <CardContent className="flex items-center justify-between gap-3 p-4">
          <div className="flex min-w-0 items-center gap-3">
            <Rss aria-hidden="true" className="size-5 shrink-0 text-muted-foreground" />
            <div className="min-w-0">
              <p className="truncate text-sm font-medium">{t("home.subscriptionCard.none")}</p>
              <p className="truncate text-xs text-muted-foreground">
                {t("home.subscriptionCard.noneHint")}
              </p>
            </div>
          </div>
          <Button onClick={onAddSubscription} size="sm" type="button">
            <Plus aria-hidden="true" className="size-4" />
            {t("home.subscriptionCard.add")}
          </Button>
        </CardContent>
      </Card>
    );
  }

  async function handleUpdate() {
    if (!subscription || updating) {
      return;
    }
    setUpdating(true);
    try {
      // `update_subscriptions` emits subscriptions + subscriptionMetadata +
      // profiles for us.
      const result = await updateSubscriptions(subscription.id, true, null);
      // A dead URL or an expired token comes back as a *successful* command
      // carrying `skipped` + `messages`, so success has to be read from the
      // payload rather than from the absence of a rejection.
      if (isSubscriptionUpdateFailure(result)) {
        pushToast({
          description:
            subscriptionUpdateMessages(result) || t("panes.subscriptions.updateNothingImported"),
          severity: "error",
          title: t("home.subscriptionCard.updateFailed"),
        });

        return;
      }
      pushToast({
        description: t("panes.subscriptions.updateResult", {
          imported: result.imported,
          updated: result.updated,
        }),
        severity: "info",
        title: t("home.subscriptionCard.updated"),
      });
    } catch (error) {
      pushToast({
        description: redactOperationalError(error),
        severity: "error",
        title: t("home.subscriptionCard.updateFailed"),
      });
    } finally {
      setUpdating(false);
    }
  }

  const ratio = usageRatio(metadata);
  const displayName = metadata?.profileTitle?.trim() || subscription.remarks || t("panes.subscriptions.untitled");

  return (
    <Card data-testid="home-subscription-card" className="rounded-2xl bg-surface-raised shadow-raised">
      <CardContent className="grid gap-3 p-4">
        <div className="flex items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-3">
            <Rss aria-hidden="true" className="size-5 shrink-0 text-muted-foreground" />
            <p className="truncate text-sm font-medium">{displayName}</p>
          </div>
          <Button
            aria-busy={updating || undefined}
            aria-label={t("home.subscriptionCard.update")}
            disabled={updating}
            onClick={() => void handleUpdate()}
            size="icon"
            type="button"
            variant="ghost"
          >
            {updating ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <RefreshCw aria-hidden="true" className="size-4" />
            )}
          </Button>
        </div>
        <SubscriptionMetaLine language={language} metadata={metadata} t={t} />
        {ratio != null ? (
          <div aria-hidden="true" className="h-1 overflow-hidden rounded-full bg-muted">
            <div
              className={cn(
                "h-full rounded-full transition-[width]",
                isTrafficExhausted(metadata) ? "bg-destructive" : "bg-brand",
              )}
              style={{ width: `${Math.round(ratio * 100)}%` }}
            />
          </div>
        ) : null}
      </CardContent>
    </Card>
  );
}

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
      label: t("home.subscriptionCard.remainingTraffic", { amount: formatBytes(remaining) }),
    });
  }
  const days = remainingDays(metadata?.expireAt);
  if (days != null) {
    stats.push(
      isExpired(metadata?.expireAt)
        ? { destructive: true, key: "days", label: t("home.subscriptionCard.expired") }
        : { key: "days", label: t("home.subscriptionCard.remainingDays", { days }) },
    );
  }
  if (metadata?.lastUpdateAt != null) {
    const { unit, value } = relativeTimeFrom(metadata.lastUpdateAt);
    const formatted = new Intl.RelativeTimeFormat(language, { numeric: "auto" }).format(value, unit);
    stats.push({
      key: "updated",
      label: t("home.subscriptionCard.lastUpdated", { time: formatted }),
    });
  }
  if (stats.length === 0) {
    return null;
  }

  return (
    <p className={cn("flex flex-wrap items-center gap-x-2 text-xs text-muted-foreground", className)}>
      {stats.map((stat, index) => (
        <span className="inline-flex items-center gap-x-2" key={stat.key}>
          {index > 0 ? <span aria-hidden="true">·</span> : null}
          <span className={stat.destructive ? "font-medium text-destructive" : undefined}>
            {stat.label}
          </span>
        </span>
      ))}
    </p>
  );
}

function pickDisplayedSubscription(
  subscriptions: readonly Subscription[],
  activeSubscriptionId: string | null,
): Subscription | null {
  if (activeSubscriptionId) {
    const active = subscriptions.find((item) => item.id === activeSubscriptionId);
    if (active) {
      return active;
    }
  }
  return subscriptions.find((item) => item.enabled) ?? subscriptions[0] ?? null;
}
