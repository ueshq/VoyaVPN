import { queries } from "@voya/client/queries";
import { useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { cn } from "@voya/ui/lib/utils";
import type { TranslationFunction } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import type { ProxyConnectionItem } from "@voya/contracts";
import { outboundLabelKey } from "@voya/features/routing/rule-outbound";
import { connectionBytes } from "@voya/features/proxy/connection-display";
import { connectionRuleText } from "@voya/features/proxy/connection-rule";

type Field = {
  label: string;
  value: string | null | undefined;
  /** The whole value on hover when the shown one is shortened. */
  full?: string | null;
  /** Addresses and paths read better in a fixed-width face. */
  mono?: boolean;
};

export function ConnectionDetails({
  connection,
  ended,
  stale,
  canDisconnect,
  pending,
  onDisconnect,
  onClose,
  onCloseFocus,
}: {
  connection: ProxyConnectionItem | null;
  ended: boolean;
  stale: boolean;
  canDisconnect: boolean;
  pending: boolean;
  onDisconnect: () => void;
  onClose: () => void;
  onCloseFocus: () => void;
}) {
  const { t, language } = useI18n();
  const titleRef = useRef<HTMLHeadingElement>(null);
  const routingsQuery = useQuery({
    enabled: connection !== null,
    ...queries.routings,
  });
  const activeRules =
    routingsQuery.data?.find((routing) => routing.isActive)?.rules ?? null;
  const date = connection?.start ? new Date(connection.start) : null;
  const startedAt =
    date && Number.isFinite(date.getTime())
      ? date.toLocaleString(language)
      : connection?.start;
  // What it is, where it went, how it got there, and how much went through.
  const groups: Field[][] = connection
    ? [
        [
          { label: t("activity.target"), value: connection.host },
          { label: t("activity.application"), value: connection.process },
          {
            full: connection.processPath,
            label: t("activity.processPath"),
            mono: true,
            value: middleEllipsis(connection.processPath),
          },
        ],
        [
          { label: t("activity.rule"), value: connectionRuleText(connection, activeRules, t) },
          { label: t("activity.proxyChain"), value: chainText(connection.chains, t) },
        ],
        [
          { label: t("activity.sourceAddress"), mono: true, value: connection.source },
          { label: t("activity.destinationAddress"), mono: true, value: connection.destination },
          {
            label: t("activity.protocol"),
            value: [connection.network, connection.connectionType].filter(Boolean).join(" / "),
          },
          { label: t("activity.startedAt"), value: startedAt },
        ],
        [
          { label: t("sidebar.upload"), value: connectionBytes(connection.upload) },
          { label: t("sidebar.download"), value: connectionBytes(connection.download) },
        ],
      ]
    : [];

  return (
    <Dialog
      open={connection !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <ScrollableDialogContent
        width="35rem"
        closeLabel={t("actions.close")}
        // The footer carries Close next to the destructive action.
        showCloseButton={false}
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          titleRef.current?.focus();
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          onCloseFocus();
        }}
      >
        <DialogHeader>
          <DialogTitle className="outline-none" tabIndex={-1} ref={titleRef}>
            {t("activity.connectionDetails")}
          </DialogTitle>
          {/* A live connection needs no subtitle; an ended or stale one says so. */}
          <DialogDescription className={ended || stale ? undefined : "sr-only"}>
            {ended
              ? t("activity.ended")
              : stale
                ? t("activity.previousData")
                : t("activity.liveConnections")}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <dl className="grid select-text gap-3 text-sm">
            {groups.flatMap((group, groupIndex) =>
              group.map((field, fieldIndex) => (
                <div
                  className={cn(
                    "grid grid-cols-[7rem_minmax(0,1fr)] gap-4",
                    // Each group after the first starts under a hairline.
                    groupIndex > 0 &&
                      fieldIndex === 0 &&
                      "mt-1 border-t border-border-subtle pt-4",
                  )}
                  key={field.label}
                >
                  <dt className="text-muted-foreground">{field.label}</dt>
                  <dd
                    className={cn(
                      "whitespace-pre-wrap [overflow-wrap:anywhere]",
                      field.mono && "font-mono text-xs leading-5",
                    )}
                    title={field.full ?? undefined}
                  >
                    {field.value?.trim() || "—"}
                  </dd>
                </div>
              )),
            )}
          </dl>
        </DialogBody>
        <DialogFooter>
          <Button onClick={onClose} type="button" variant="outline">
            {t("actions.close")}
          </Button>
          <Button
            disabled={!canDisconnect || ended || pending}
            onClick={onDisconnect}
            variant="destructive"
            type="button"
          >
            {t("activity.disconnectOne")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}

function chainText(chains: readonly string[], t: TranslationFunction) {
  return chains
    .map((tag) => {
      const key = outboundLabelKey(tag.toLowerCase());
      return key ? t(key) : tag;
    })
    .join(" → ");
}

/** A long path keeps its start and its file name; the full path is on hover. */
function middleEllipsis(text: string | null | undefined, max = 64) {
  if (!text || text.length <= max) return text;
  const keep = Math.floor((max - 1) / 2);
  return `${text.slice(0, keep)}…${text.slice(-keep)}`;
}
