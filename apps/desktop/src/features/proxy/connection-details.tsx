import { useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogBody,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import type { TranslationFunction, TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { listRoutings } from "@/ipc/commands";
import type { ProxyConnectionItem } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { connectionBytes } from "./connection-display";
import { connectionRuleText } from "./connection-rule";

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
    queryFn: listRoutings,
    queryKey: queryKeys.routings,
  });
  const activeRules =
    routingsQuery.data?.find((routing) => routing.isActive)?.rules ?? null;
  const date = connection?.start ? new Date(connection.start) : null;
  const startedAt =
    date && Number.isFinite(date.getTime())
      ? date.toLocaleString(language)
      : connection?.start;
  const fields = connection
    ? [
        [t("activity.target"), connection.host],
        [t("activity.application"), connection.process],
        [t("activity.processPath"), middleEllipsis(connection.processPath), connection.processPath],
        [t("activity.sourceAddress"), connection.source],
        [t("activity.destinationAddress"), connection.destination],
        [
          t("activity.protocol"),
          [connection.network, connection.connectionType]
            .filter(Boolean)
            .join(" / "),
        ],
        [t("activity.startedAt"), startedAt],
        [t("activity.rule"), connectionRuleText(connection, activeRules, t)],
        [t("activity.proxyChain"), chainText(connection.chains, t)],
        [t("sidebar.upload"), connectionBytes(connection.upload)],
        [t("sidebar.download"), connectionBytes(connection.download)],
      ]
    : [];

  return (
    <Dialog
      open={connection !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent
        className="flex max-h-[85vh] flex-col sm:max-w-[560px]"
        closeLabel={t("actions.close")}
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
          <DialogDescription>
            {ended
              ? t("activity.ended")
              : stale
                ? t("activity.previousData")
                : t("activity.liveConnections")}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <dl className="select-text space-y-4 text-sm">
            {fields.map(([label, value, fullText]) => (
              <div
                className="grid grid-cols-[7rem_minmax(0,1fr)] gap-4"
                key={label}
              >
                <dt className="text-muted-foreground">{label}</dt>
                <dd className="whitespace-pre-wrap [overflow-wrap:anywhere]" title={fullText ?? undefined}>
                  {value?.trim() || "—"}
                </dd>
              </div>
            ))}
          </dl>
        </DialogBody>
        <DialogFooter>
          <Button
            disabled={!canDisconnect || ended || pending}
            onClick={onDisconnect}
            variant="destructive"
            type="button"
          >
            {t("activity.disconnectOne")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// The core names its built-in outbounds by tag; the Rules page has words for them.
const OUTBOUND_KEYS: Record<string, TranslationKey> = {
  block: "panes.routing.outboundBlock",
  direct: "panes.routing.outboundDirect",
  proxy: "panes.routing.outboundProxy",
};

function chainText(chains: readonly string[], t: TranslationFunction) {
  return chains
    .map((tag) => {
      const key = OUTBOUND_KEYS[tag.toLowerCase()];
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
