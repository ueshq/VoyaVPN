import { useRef } from "react";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import { useI18n } from "@voya/i18n/use-i18n";
import { connectionBytes } from "./connection-display";
import type { ProxyConnectionItem } from "@/ipc/bindings";

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
  const date = connection?.start ? new Date(connection.start) : null;
  const startedAt = date && Number.isFinite(date.getTime()) ? date.toLocaleString(language) : connection?.start;
  const fields = connection
    ? [
        [t("activity.target"), connection.host],
        [t("activity.application"), connection.process],
        [t("activity.processPath"), connection.processPath],
        [t("activity.sourceAddress"), connection.source],
        [t("activity.destinationAddress"), connection.destination],
        [t("activity.protocol"), [connection.network, connection.connectionType].filter(Boolean).join(" / ")],
        [t("activity.startedAt"), startedAt],
        [t("activity.rule"), [connection.rule, connection.rulePayload].filter(Boolean).join(" · ")],
        [t("activity.proxyChain"), connection.chains.join(" → ")],
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
            {t(ended ? "activity.ended" : stale ? "activity.previousData" : "activity.liveConnections")}
          </DialogDescription>
        </DialogHeader>
        <dl className="min-h-0 select-text space-y-4 overflow-y-auto px-6 py-5 text-sm">
          {fields.map(([label, value]) => (
            <div className="grid grid-cols-[7rem_minmax(0,1fr)] gap-4" key={label}>
              <dt className="text-muted-foreground">{label}</dt>
              <dd className="whitespace-pre-wrap [overflow-wrap:anywhere]">{value?.trim() || "—"}</dd>
            </div>
          ))}
        </dl>
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
