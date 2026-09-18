import { useEffect, useState } from "react";
import { Check, Copy, KeyRound, QrCode } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { ConfirmDialog } from "@voya/ui/components/confirm-dialog";

import { ShareQrDialog } from "@/features/profiles/share-qr-dialog";
import { SettingsGroup } from "@/features/settings/settings-form";
import type { SelfHostShareLink, SelfHostState } from "@/ipc/bindings";
import { writeClipboard } from "@/lib/clipboard";
import { toastError } from "@/stores/toast-store";

import { ADDRESS_KIND_KEYS } from "./self-host-labels";
import type { SelfHostController } from "./use-self-host";

const COPIED_FEEDBACK_MS = 1500;
const ALL_LINKS = "all";

export function ShareLinksCard({
  controller,
  state,
}: {
  controller: SelfHostController;
  state: SelfHostState;
}) {
  const { t } = useI18n();
  const [qrContent, setQrContent] = useState<string | null>(null);
  const [confirmRotate, setConfirmRotate] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);
  const links = state.shareLinks;

  useEffect(() => {
    if (copied === null) return;
    const timer = window.setTimeout(() => setCopied(null), COPIED_FEEDBACK_MS);
    return () => window.clearTimeout(timer);
  }, [copied]);

  async function copy(text: string, key: string) {
    try {
      await writeClipboard(text);
      setCopied(key);
    } catch (error) {
      toastError(t("panes.selfHost.actionFailed"), error);
    }
  }

  return (
    <SettingsGroup
      actions={
        links.length > 0 ? (
          <>
            <Button
              onClick={() => void copy(links.map((link) => link.link).join("\n"), ALL_LINKS)}
              size="sm"
              type="button"
              variant="outline"
            >
              {copied === ALL_LINKS ? <Check aria-hidden="true" className="size-4" /> : <Copy aria-hidden="true" className="size-4" />}
              {t(copied === ALL_LINKS ? "panes.selfHost.links.copied" : "panes.selfHost.links.copyAll")}
            </Button>
            <Button
              disabled={controller.pending === "rotate"}
              onClick={() => setConfirmRotate(true)}
              size="sm"
              type="button"
              variant="outline"
            >
              <KeyRound aria-hidden="true" className="size-4" />
              {t("panes.selfHost.links.rotate")}
            </Button>
          </>
        ) : null
      }
      title={t("panes.selfHost.links.title")}
    >
      <p className="text-sm text-muted-foreground">
        {t(links.length > 0 ? "panes.selfHost.links.hint" : "panes.selfHost.links.emptyHint")}
      </p>
      {links.length > 0 && state.runtime.status !== "running" ? (
        <p className="text-sm text-warning">{t("panes.selfHost.links.notRunning")}</p>
      ) : null}
      {links.map((link) => (
        <LinkRow
          copied={copied === link.link}
          key={link.link}
          link={link}
          onCopy={() => void copy(link.link, link.link)}
          onShowQr={() => setQrContent(link.link)}
        />
      ))}
      <ShareQrDialog
        content={qrContent ?? ""}
        onOpenChange={(open) => {
          if (!open) setQrContent(null);
        }}
        open={qrContent !== null}
      />
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
    </SettingsGroup>
  );
}

function LinkRow({
  copied,
  link,
  onCopy,
  onShowQr,
}: {
  copied: boolean;
  link: SelfHostShareLink;
  onCopy: () => void;
  onShowQr: () => void;
}) {
  const { t } = useI18n();
  const protocol = t(link.protocol === "vless" ? "panes.selfHost.config.vless" : "panes.selfHost.config.shadowsocks");
  const host = link.address.includes(":") ? `[${link.address}]` : link.address;

  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-2" data-testid="self-host-link">
      <Badge variant="secondary">{protocol}</Badge>
      <span className="text-sm text-muted-foreground">{t(ADDRESS_KIND_KEYS[link.addressKind])}</span>
      <span className="min-w-0 truncate font-mono text-sm">{`${host}:${link.port}`}</span>
      <div className="ms-auto flex items-center gap-2">
        <Button
          aria-label={t("panes.selfHost.links.showQr")}
          onClick={onShowQr}
          size="sm"
          title={t("panes.selfHost.links.showQr")}
          type="button"
          variant="ghost"
        >
          <QrCode aria-hidden="true" className="size-4" />
        </Button>
        <Button onClick={onCopy} size="sm" type="button" variant="outline">
          {copied ? <Check aria-hidden="true" className="size-4" /> : <Copy aria-hidden="true" className="size-4" />}
          {t(copied ? "panes.selfHost.links.copied" : "panes.selfHost.links.copy")}
        </Button>
      </div>
    </div>
  );
}
