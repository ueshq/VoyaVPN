import { useRef } from "react";
import {
  ChevronDown,
  FilePlus2,
  FileWarning,
  Rss,
  Share2,
  Upload,
} from "lucide-react";

import { Toolbar, ToolbarGroup } from "@/components/app-shell/toolbar";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import {
  PageHeader,
  PageHeaderActions,
} from "@/components/app-shell/page-section";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import {
  Menubar,
  MenubarContent,
  MenubarMenu,
  MenubarItem,
  MenubarTrigger,
} from "@voya/ui/components/menubar";
import { getErrorMessage } from "@voya/utils/error";

import { IMPORT_METHODS } from "./import-methods";
import { ExportMenuItems, SpeedtestButton } from "./server-table-menus";
import type { ServerTableController } from "./use-server-table";

export function ServerTableToolbar({
  controller,
}: {
  controller: ServerTableController;
}) {
  const {
    nodeGroups,
    handleBulkExport,
    handleCancelSpeedtest,
    handleSpeedtest,
    addTriggerRef,
    importTriggerRef,
    operationError,
    operationMessage,
    profiles,
    profilesQuery,
    setDialogState,
    setImportMethod,
    openSubscription,
    speedtestRunning,
    t,
    undecodableProfiles,
  } = controller;
  const openingDialogRef = useRef(false);
  function handleMenuClose(event: Event) {
    if (openingDialogRef.current) {
      event.preventDefault();
      openingDialogRef.current = false;
    }
  }
  const batchActionsDisabled = profilesQuery.isLoading || profiles.length === 0;

  return (
    <>
      <PageHeader className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex shrink-0 flex-wrap items-center gap-2">
          <Badge
            className="h-6 bg-background tabular-nums text-muted-foreground"
            variant="outline"
          >
            {t("panes.profiles.toolbar.rows", {
              rows: profiles.length.toLocaleString(),
            })}
          </Badge>
          <Badge
            className="h-6 bg-background tabular-nums text-muted-foreground"
            variant="outline"
          >
            {t("nodeGroups.groupCount", {
              count: controller.rows.filter((row) => row.kind === "group").length,
            })}
          </Badge>
        </div>

        <PageHeaderActions className="min-w-0 max-w-full">
          <Toolbar className="min-w-0 max-w-full justify-end">
            <ToolbarGroup className="min-w-0 flex-wrap justify-end gap-y-2">
              <Button
                size="sm"
                onClick={(event) => openSubscription(null, event.currentTarget)}
              >
                <Rss aria-hidden="true" className="size-4" />
                {t("home.subscriptionCard.add")}
              </Button>
              <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
                <MenubarMenu>
                  <MenubarTrigger asChild className="h-8">
                    <Button
                      ref={addTriggerRef}
                      size="sm"
                      type="button"
                      variant="outline"
                    >
                      <FilePlus2 className="size-4" aria-hidden="true" />
                      {t("panes.profiles.toolbar.add")}
                      <ChevronDown className="size-3" aria-hidden="true" />
                    </Button>
                  </MenubarTrigger>
                  <MenubarContent onCloseAutoFocus={handleMenuClose}>
                    <MenubarItem
                      onSelect={() => {
                        openingDialogRef.current = true;
                        setDialogState({ mode: "create" });
                      }}
                    >
                      <FilePlus2 aria-hidden="true" />
                      {t("panes.profiles.dialog.addTitle")}
                    </MenubarItem>
                    <MenubarItem
                      onSelect={() => {
                        openingDialogRef.current = true;
                        nodeGroups.open(
                          { kind: "name", group: null },
                          addTriggerRef.current,
                        );
                      }}
                    >
                      {t("nodeGroups.create")}
                    </MenubarItem>
                  </MenubarContent>
                </MenubarMenu>
              </Menubar>
              <SpeedtestButton
                disabled={batchActionsDisabled}
                label={t("panes.profiles.toolbar.bulkSpeedtest")}
                onCancel={handleCancelSpeedtest}
                onRun={() => handleSpeedtest({ scope: "all" })}
                running={speedtestRunning}
              />
              <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
                <MenubarMenu>
                  <MenubarTrigger asChild className="h-8">
                    <Button
                      disabled={batchActionsDisabled}
                      size="sm"
                      type="button"
                      variant="outline"
                    >
                      <Share2 className="size-4" aria-hidden="true" />
                      {t("panes.profiles.toolbar.bulkExport")}
                    </Button>
                  </MenubarTrigger>
                  <MenubarContent align="start">
                    <ExportMenuItems
                      onExport={(kind) => void handleBulkExport(kind)}
                      onSave={(kind) =>
                        void handleBulkExport(kind, false, true)
                      }
                      onShowQr={() => void handleBulkExport("shareLinks", true)}
                      t={t}
                    />
                  </MenubarContent>
                </MenubarMenu>
              </Menubar>
            </ToolbarGroup>

            <ToolbarGroup className="min-w-0 flex-wrap justify-end gap-y-2">
              <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
                <MenubarMenu>
                  <MenubarTrigger asChild className="h-8">
                    <Button
                      ref={importTriggerRef}
                      size="sm"
                      type="button"
                      variant="outline"
                    >
                      <Upload className="size-4" aria-hidden="true" />
                      {t("panes.profiles.toolbar.import")}
                      <ChevronDown className="size-3" aria-hidden="true" />
                    </Button>
                  </MenubarTrigger>
                  <MenubarContent onCloseAutoFocus={handleMenuClose}>
                    {IMPORT_METHODS.map(({ method, icon: Icon, labelKey }) => (
                      <MenubarItem
                        key={method}
                        onSelect={() => {
                          openingDialogRef.current = true;
                          setImportMethod(method);
                        }}
                      >
                        <Icon aria-hidden="true" />
                        {t(labelKey)}
                      </MenubarItem>
                    ))}
                  </MenubarContent>
                </MenubarMenu>
              </Menubar>
            </ToolbarGroup>
          </Toolbar>
        </PageHeaderActions>
      </PageHeader>

      {nodeGroups.query.error ? (
        <InlinePageError>
          {getErrorMessage(nodeGroups.query.error)}
        </InlinePageError>
      ) : null}
      {!nodeGroups.dialog && nodeGroups.error ? (
        <InlinePageError>{nodeGroups.error}</InlinePageError>
      ) : null}
      {operationError ? (
        <InlinePageError>{operationError}</InlinePageError>
      ) : null}
      {profilesQuery.isError ? (
        <InlinePageError>
          {getErrorMessage(profilesQuery.error)}
        </InlinePageError>
      ) : null}
      {operationMessage ? (
        <div className="border-b bg-connected/10 px-4 py-2 text-sm text-success">
          {operationMessage}
        </div>
      ) : null}
      {/*
        Stored profiles this build could not decode are skipped by persistence so
        that one of them cannot hide every other server. This band is the only
        place the user hears about it: a toast would fire on every refetch of the
        list, so the shortfall is stated calmly next to the rows instead, and it
        stays until the profiles become readable again.
      */}
      {undecodableProfiles > 0 ? (
        <div
          className="flex items-start gap-2 border-b bg-muted/40 px-4 py-2 text-sm text-muted-foreground"
          data-slot="profiles-undecodable-notice"
          role="status"
        >
          <FileWarning aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
          <span>
            {t("panes.profiles.undecodable", {
              count: undecodableProfiles.toLocaleString(),
            })}
          </span>
        </div>
      ) : null}
    </>
  );
}
