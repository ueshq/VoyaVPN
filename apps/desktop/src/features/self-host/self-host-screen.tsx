import { RefreshCw } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import { Skeleton } from "@voya/ui/components/skeleton";
import { Spinner } from "@voya/ui/components/spinner";
import { Switch } from "@voya/ui/components/switch";

import { InlinePageError } from "@/components/app-shell/inline-page-error";
import {
  PageContent,
  PageSection,
  PageSurface,
  PageTitle,
} from "@/components/app-shell/page-section";

import { ConnectCard } from "./connect-card";
import { EnvironmentCard } from "./environment-card";
import { NodeConfigCard } from "./node-config-card";
import { ShareLinksCard } from "./share-links-card";
import { StatusCard } from "./status-card";
import { useSelfHost } from "./use-self-host";

const ENABLE_SWITCH_ID = "self-host-enabled";

/**
 * Self-hosted node: this device serves as an exit for other devices. Status
 * and links come first because that is what a returning user looks for; the
 * network check explains whether others can reach it, and settings follow.
 */
export function SelfHostScreen() {
  const { t } = useI18n();
  const controller = useSelfHost();
  const { pending, state } = controller;
  const checking = pending === "check";

  return (
    <PageSection aria-label={t("tabs.selfHost")}>
      <PageTitle
        actions={
          <>
            <Button
              disabled={!state || checking}
              onClick={() => void controller.runCheck()}
              size="sm"
              type="button"
              variant="outline"
            >
              {checking ? (
                <Spinner aria-hidden="true" className="size-4" />
              ) : (
                <RefreshCw aria-hidden="true" className="size-4" />
              )}
              {t(checking ? "panes.selfHost.network.checking" : "panes.selfHost.network.run")}
            </Button>
            <label className="text-sm font-medium" htmlFor={ENABLE_SWITCH_ID}>
              {t("panes.selfHost.enable")}
            </label>
            <Switch
              checked={state?.config.enabled ?? false}
              disabled={!state || pending === "enable"}
              id={ENABLE_SWITCH_ID}
              onCheckedChange={(enabled) => void controller.setEnabled(enabled)}
            />
          </>
        }
        title={t("tabs.selfHost")}
      />
      <PageContent>
        {controller.loadError ? <InlinePageError>{controller.loadError}</InlinePageError> : null}
        <ScrollArea className="min-h-0 min-w-0 flex-1 [&_[data-slot=scroll-area-viewport]>div]:block!">
          {state ? (
            <div className="grid gap-4">
              <StatusCard state={state} stats={controller.stats} />
              <ShareLinksCard controller={controller} state={state} />
              <EnvironmentCard controller={controller} state={state} />
              <NodeConfigCard controller={controller} state={state} />
              <ConnectCard />
            </div>
          ) : (
            <div className="grid gap-4" aria-busy="true">
              <PageSurface className="p-4">
                <Skeleton className="h-6 w-40" />
              </PageSurface>
              <PageSurface className="p-4">
                <Skeleton className="h-24 w-full" />
              </PageSurface>
            </div>
          )}
        </ScrollArea>
      </PageContent>
    </PageSection>
  );
}
