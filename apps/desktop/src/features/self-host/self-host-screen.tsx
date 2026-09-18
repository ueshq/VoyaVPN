import { useState } from "react";
import { Link2, Settings2 } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import { Skeleton } from "@voya/ui/components/skeleton";

import { InlinePageError } from "@/components/app-shell/inline-page-error";
import {
  PageContent,
  PageSection,
  PageSurface,
  PageTitle,
} from "@/components/app-shell/page-section";
import type { SelfHostState } from "@/ipc/bindings";

import { ActionTile } from "./action-tile";
import { HostingCard } from "./hosting-card";
import { NodeSettingsDialog } from "./node-settings-dialog";
import { ShareLinksDialog } from "./share-links-dialog";
import { useSelfHost } from "./use-self-host";

type SelfHostDialog = "links" | "settings";

/**
 * Self-hosted node: this device serves as an exit for other devices. The
 * hosting card answers whether the node runs and whether others can reach it;
 * links and settings are one click away in a dialog whose tile already shows
 * where it stands.
 */
export function SelfHostScreen() {
  const { t } = useI18n();
  const controller = useSelfHost();
  const { state } = controller;
  const [dialog, setDialog] = useState<SelfHostDialog | null>(null);
  const close = (open: boolean) => {
    if (!open) setDialog(null);
  };

  return (
    <PageSection aria-label={t("tabs.selfHost")}>
      <PageTitle title={t("tabs.selfHost")} />
      <PageContent>
        {controller.loadError ? <InlinePageError>{controller.loadError}</InlinePageError> : null}
        <ScrollArea className="min-h-0 min-w-0 flex-1 [&_[data-slot=scroll-area-viewport]>div]:block!">
          {state ? (
            <div className="grid gap-4">
              <HostingCard
                controller={controller}
                onOpenSettings={() => setDialog("settings")}
                state={state}
              />
              <div className="grid grid-cols-2 gap-4">
                <ActionTile
                  icon={Link2}
                  onClick={() => setDialog("links")}
                  summary={t(linksSummaryKey(state))}
                  title={t("panes.selfHost.links.title")}
                />
                <ActionTile
                  icon={Settings2}
                  onClick={() => setDialog("settings")}
                  summary={protocolSummary(state) ?? t("panes.selfHost.config.noProtocol")}
                  title={t("panes.selfHost.config.title")}
                />
              </div>
            </div>
          ) : (
            <div className="grid gap-4" aria-busy="true">
              <PageSurface className="p-5">
                <Skeleton className="h-12 w-full" />
              </PageSurface>
              <div className="grid grid-cols-2 gap-4">
                <Skeleton className="h-20 rounded-xl" />
                <Skeleton className="h-20 rounded-xl" />
              </div>
            </div>
          )}
        </ScrollArea>
      </PageContent>
      {state && dialog === "links" ? (
        <ShareLinksDialog controller={controller} onOpenChange={close} state={state} />
      ) : null}
      {state && dialog === "settings" ? (
        <NodeSettingsDialog controller={controller} onOpenChange={close} state={state} />
      ) : null}
    </PageSection>
  );
}

function linksSummaryKey(state: SelfHostState) {
  if (state.shareLinks.length === 0) return "panes.selfHost.links.summaryEmpty";
  return state.runtime.status === "running"
    ? "panes.selfHost.links.summaryReady"
    : "panes.selfHost.links.summaryNotRunning";
}

/** The enabled protocols by their short names, which no locale translates. */
function protocolSummary({ config }: SelfHostState) {
  const names = [config.vlessEnabled ? "VLESS" : null, config.shadowsocksEnabled ? "Shadowsocks" : null];
  return names.filter(Boolean).join(" · ") || null;
}
