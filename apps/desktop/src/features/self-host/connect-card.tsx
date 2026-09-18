import { ClipboardPaste, Monitor, Plus, Server } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { Spinner } from "@voya/ui/components/spinner";

import { formatImportSummary } from "@/features/profiles/server-table-actions";
import { useNodeImport } from "@/features/profiles/use-node-import";
import { useNodeOperation } from "@/features/profiles/use-node-operation";
import { SettingsGroup } from "@/features/settings/settings-form";
import { useShellStore } from "@/stores/shell-store";

/**
 * Using a node another device hosts is ordinary node import: its links are
 * standard VLESS and Shadowsocks share links. This card is the shortcut from
 * the page people look at when they think "self-hosted".
 */
export function ConnectCard() {
  const { t } = useI18n();
  const operation = useNodeOperation();
  const { directImportPending, handleDirectImport } = useNodeImport(
    operation,
    async (result) => {
      operation.setOperationMessage(formatImportSummary(result, t));
    },
    t,
  );
  const busy = directImportPending !== null;

  return (
    <SettingsGroup title={t("panes.selfHost.connect.title")}>
      <p className="text-sm text-muted-foreground">{t("panes.selfHost.connect.description")}</p>
      <div className="flex flex-wrap items-center gap-2">
        <Button
          disabled={busy}
          onClick={() => void handleDirectImport("clipboard")}
          size="sm"
          type="button"
          variant="outline"
        >
          {directImportPending === "clipboard" ? (
            <Spinner aria-hidden="true" className="size-4" />
          ) : (
            <ClipboardPaste aria-hidden="true" className="size-4" />
          )}
          {t("panes.selfHost.connect.paste")}
        </Button>
        <Button
          disabled={busy}
          onClick={() => void handleDirectImport("qrScreen")}
          size="sm"
          type="button"
          variant="outline"
        >
          {directImportPending === "qrScreen" ? (
            <Spinner aria-hidden="true" className="size-4" />
          ) : (
            <Monitor aria-hidden="true" className="size-4" />
          )}
          {t("panes.selfHost.connect.scan")}
        </Button>
        <Button
          onClick={() => useShellStore.getState().openProfilesAddMenu()}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Plus aria-hidden="true" className="size-4" />
          {t("panes.selfHost.connect.more")}
        </Button>
      </div>
      {operation.operationMessage ? (
        <div className="flex flex-wrap items-center gap-2" role="status">
          <span className="text-sm">{operation.operationMessage}</span>
          <Button
            onClick={() => useShellStore.getState().setActiveTab("profiles")}
            size="sm"
            type="button"
            variant="ghost"
          >
            <Server aria-hidden="true" className="size-4" />
            {t("panes.selfHost.connect.goToNodes")}
          </Button>
        </div>
      ) : null}
      {operation.operationError ? (
        <p className="whitespace-pre-line text-sm text-danger" role="alert">
          {operation.operationError}
        </p>
      ) : null}
    </SettingsGroup>
  );
}
