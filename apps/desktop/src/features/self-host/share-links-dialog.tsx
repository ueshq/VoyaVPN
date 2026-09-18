import { useEffect, useState } from "react";
import { Check, Copy, KeyRound, RefreshCw, ShieldAlert } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { ConfirmDialog } from "@voya/ui/components/confirm-dialog";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { Input } from "@voya/ui/components/input";
import { Spinner } from "@voya/ui/components/spinner";
import { cn } from "@voya/ui/lib/utils";

import { ShareQrImage } from "@/features/profiles/share-qr-dialog";
import type { SelfHostShareLink, SelfHostState } from "@/ipc/bindings";
import { writeClipboard } from "@/lib/clipboard";
import { toastError } from "@/stores/toast-store";

import { ADDRESS_KIND_KEYS } from "./self-host-labels";
import type { SelfHostController } from "./use-self-host";

const COPIED_FEEDBACK_MS = 1500;

/**
 * Handing the node to another device: every address shows its link with a Copy
 * button, and the selected one's QR code sits alongside. One link is always
 * selected, so the QR code is there the moment the dialog opens.
 */
export function ShareLinksDialog({
  controller,
  onOpenChange,
  state,
}: {
  controller: SelfHostController;
  onOpenChange: (open: boolean) => void;
  state: SelfHostState;
}) {
  const { t } = useI18n();
  const [selectedLink, setSelectedLink] = useState<string | null>(null);
  const [confirmRotate, setConfirmRotate] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);
  const links = state.shareLinks;
  // Reset keys replaces every link, so a selection that is gone falls back.
  const selected = links.find((link) => link.link === selectedLink) ?? links[0] ?? null;

  useEffect(() => {
    if (copied === null) return;
    const timer = window.setTimeout(() => setCopied(null), COPIED_FEEDBACK_MS);
    return () => window.clearTimeout(timer);
  }, [copied]);

  async function copy(link: string) {
    try {
      await writeClipboard(link);
      setCopied(link);
    } catch (error) {
      toastError(t("panes.selfHost.actionFailed"), error);
    }
  }

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <ScrollableDialogContent closeLabel={t("actions.close")} width="40rem">
        <DialogHeader>
          <DialogTitle>{t("panes.selfHost.links.title")}</DialogTitle>
          <DialogDescription>{t("panes.selfHost.links.hint")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="grid gap-4">
          {selected ? (
            <>
              {state.runtime.status === "running" ? null : (
                <p className="text-sm text-warning" role="status">
                  {t("panes.selfHost.links.notRunning")}
                </p>
              )}
              <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_13rem] sm:items-start">
                <ul className="grid gap-2">
                  {links.map((link) => (
                    <li key={link.link}>
                      <LinkOption
                        copied={copied === link.link}
                        link={link}
                        onCopy={() => void copy(link.link)}
                        onSelect={() => setSelectedLink(link.link)}
                        selected={link === selected}
                      />
                    </li>
                  ))}
                </ul>
                {/* Sticks while a long list scrolls, so the code stays next to the row. */}
                <div className="sm:sticky sm:top-0">
                  <ShareQrImage className="size-44" content={selected.link} />
                </div>
              </div>
              <p className="flex items-start gap-1.5 text-xs text-muted-foreground">
                <ShieldAlert aria-hidden="true" className="mt-0.5 size-3.5 shrink-0 text-warning" />
                {t("panes.selfHost.links.trustWarning")}
              </p>
            </>
          ) : (
            <div className="grid justify-items-start gap-3 py-2">
              <p className="text-sm text-muted-foreground">{t("panes.selfHost.links.emptyHint")}</p>
              {/* Checking here keeps the dialog open: links appear as soon as an address is found. */}
              <Button
                disabled={controller.pending === "check"}
                onClick={() => void controller.runCheck()}
                type="button"
                variant="outline"
              >
                {controller.pending === "check" ? (
                  <Spinner aria-hidden="true" className="size-4" />
                ) : (
                  <RefreshCw aria-hidden="true" className="size-4" />
                )}
                {controller.pending === "check"
                  ? t("panes.selfHost.network.checking")
                  : t("panes.selfHost.network.title")}
              </Button>
            </div>
          )}
        </DialogBody>
        <DialogFooter className="items-center sm:justify-between">
          {links.length > 0 ? (
            <Button
              disabled={controller.pending === "rotate"}
              onClick={() => setConfirmRotate(true)}
              type="button"
              variant="outline"
            >
              <KeyRound aria-hidden="true" className="size-4" />
              {t("panes.selfHost.links.rotate")}
            </Button>
          ) : (
            <span />
          )}
          <Button onClick={() => onOpenChange(false)} type="button">
            {t("actions.done")}
          </Button>
        </DialogFooter>
        <ConfirmDialog
          cancelLabel={t("actions.cancel")}
          confirmLabel={t("panes.selfHost.links.rotate")}
          description={t("panes.selfHost.links.rotateDescription")}
          destructive
          onConfirm={() => {
            setConfirmRotate(false);
            void controller.rotateCredentials();
          }}
          onOpenChange={setConfirmRotate}
          open={confirmRotate}
          title={t("panes.selfHost.links.rotateTitle")}
        />
      </ScrollableDialogContent>
    </Dialog>
  );
}

/**
 * One address: its header selects it for the QR code, and its link sits below
 * in a read-only field with Copy. Using the field or Copy selects it too, so
 * the QR code always matches the link just handled.
 */
function LinkOption({
  copied,
  link,
  onCopy,
  onSelect,
  selected,
}: {
  copied: boolean;
  link: SelfHostShareLink;
  onCopy: () => void;
  onSelect: () => void;
  selected: boolean;
}) {
  const { t } = useI18n();
  const protocol = t(link.protocol === "vless" ? "panes.selfHost.config.vless" : "panes.selfHost.config.shadowsocks");
  const host = link.address.includes(":") ? `[${link.address}]` : link.address;

  return (
    <div
      className={cn(
        "grid min-w-0 gap-2 rounded-md border p-3 transition-colors",
        selected ? "border-primary bg-surface-hovered" : "border-border-subtle hover:bg-surface-hovered",
      )}
      data-testid="self-host-link"
    >
      <button
        aria-pressed={selected}
        className="grid min-w-0 gap-1 rounded-sm text-start outline-none focus-visible:ring-2 focus-visible:ring-ring"
        onClick={onSelect}
        type="button"
      >
        <span className="flex min-w-0 flex-wrap items-center gap-2">
          <Badge variant="secondary">{protocol}</Badge>
          <span className="text-xs text-muted-foreground">{t(ADDRESS_KIND_KEYS[link.addressKind])}</span>
        </span>
        <span className="min-w-0 truncate font-mono text-sm">{`${host}:${link.port}`}</span>
      </button>
      <div className="flex min-w-0 items-center gap-2">
        <Input
          aria-label={t("panes.selfHost.links.link")}
          className="h-control-compact min-w-0 flex-1 bg-surface-raised font-mono text-xs text-muted-foreground md:text-xs"
          onFocus={(event) => {
            event.currentTarget.select();
            onSelect();
          }}
          readOnly
          value={link.link}
        />
        <Button
          onClick={() => {
            onSelect();
            onCopy();
          }}
          size="sm"
          type="button"
          variant="outline"
        >
          {copied ? <Check aria-hidden="true" className="size-4" /> : <Copy aria-hidden="true" className="size-4" />}
          {t(copied ? "panes.selfHost.links.copied" : "panes.selfHost.links.copy")}
        </Button>
      </div>
    </div>
  );
}
