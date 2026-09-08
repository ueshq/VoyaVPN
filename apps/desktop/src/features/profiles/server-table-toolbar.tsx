import {
  ClipboardPaste,
  Columns3,
  FilePlus2,
  Filter,
  FileWarning,
  RotateCcw,
  Rss,
  Search,
  Share2,
  Upload,
} from "lucide-react";

import { Toolbar, ToolbarGroup, ToolbarOverflow } from "@/components/app-shell/toolbar";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { PageHeader } from "@/components/app-shell/page-section";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { Input } from "@voya/ui/components/input";
import {
  Menubar,
  MenubarCheckboxItem,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarTrigger,
} from "@voya/ui/components/menubar";
import { getErrorMessage } from "@voya/utils/error";

import { COLUMN_LABEL_KEY_BY_ID } from "./server-table-columns";
import { ExportMenuItems, SpeedtestSplitButton } from "./server-table-menus";
import type { ServerTableController } from "./use-server-table";

export function ServerTableToolbar({ controller }: { controller: ServerTableController }) {
  const {
    filterText,
    handleBulkExport,
    handleCancelSpeedtest,
    handleImportFromClipboard,
    handleSpeedtest,
    hideableColumns,
    importingFromClipboard,
    operationError,
    operationMessage,
    profiles,
    profilesQuery,
    requestDedupe,
    resetColumnVisibility,
    setDialogState,
    setFilterText,
    setImportOpen,
    setSubscriptionsOpen,
    speedtestRunning,
    t,
    undecodableProfiles,
  } = controller;
  const batchActionsDisabled = profilesQuery.isLoading || (!filterText.trim() && profiles.length === 0);

  return (
    <>
      <PageHeader>
        <Badge className="h-6 bg-background tabular-nums text-muted-foreground" variant="outline">
          {t("panes.profiles.toolbar.rows", { rows: profiles.length.toLocaleString() })}
        </Badge>

        <div className="relative ms-auto min-w-[14rem]">
          <Search
            className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            aria-label={t("panes.profiles.aria.filter")}
            className="h-9 ps-9"
            onChange={(event) => setFilterText(event.target.value)}
            placeholder={t("panes.profiles.toolbar.filterPlaceholder")}
            type="search"
            value={filterText}
          />
        </div>
      </PageHeader>

      <Toolbar className="shrink-0 border-b px-4 py-2">
        <ToolbarGroup>
          <Button onClick={() => setDialogState({ mode: "create" })} size="sm" type="button">
            <FilePlus2 className="size-4" aria-hidden="true" />
            {t("panes.profiles.toolbar.add")}
          </Button>
          <SpeedtestSplitButton
            disabled={batchActionsDisabled}
            label={t("panes.profiles.toolbar.bulkSpeedtest")}
            onCancel={handleCancelSpeedtest}
            onRun={(kind) => handleSpeedtest(kind, { scope: "all" })}
            running={speedtestRunning}
          />
          <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
            <MenubarMenu>
              <MenubarTrigger asChild>
                <Button disabled={batchActionsDisabled} size="sm" type="button" variant="outline">
                  <Share2 className="size-4" aria-hidden="true" />
                  {t("panes.profiles.toolbar.bulkExport")}
                </Button>
              </MenubarTrigger>
              <MenubarContent align="start">
                <ExportMenuItems
                  onExport={(kind) => void handleBulkExport(kind)}
                  onSave={(kind) => void handleBulkExport(kind, false, true)}
                  onShowQr={() => void handleBulkExport("shareLinks", true)}
                  t={t}
                />
              </MenubarContent>
            </MenubarMenu>
          </Menubar>
        </ToolbarGroup>

        <ToolbarGroup>
          <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
            <MenubarMenu>
              <MenubarTrigger asChild>
                <Button size="sm" type="button" variant="outline">
                  <Columns3 className="size-4" aria-hidden="true" />
                  {t("panes.profiles.columns.toggle")}
                </Button>
              </MenubarTrigger>
              <MenubarContent align="end">
                <div className="px-2 py-1.5 text-xs font-medium text-muted-foreground">
                  {t("panes.profiles.columns.heading")}
                </div>
                <MenubarSeparator />
                {hideableColumns.map((column) => (
                  <MenubarCheckboxItem
                    checked={column.getIsVisible()}
                    key={column.id}
                    onCheckedChange={(value) => column.toggleVisibility(value === true)}
                    onSelect={(event) => event.preventDefault()}
                  >
                    {t(COLUMN_LABEL_KEY_BY_ID[column.id])}
                  </MenubarCheckboxItem>
                ))}
                <MenubarSeparator />
                <MenubarItem onSelect={() => resetColumnVisibility()}>
                  <RotateCcw className="size-4" aria-hidden="true" />
                  {t("panes.profiles.columns.reset")}
                </MenubarItem>
              </MenubarContent>
            </MenubarMenu>
          </Menubar>
        </ToolbarGroup>

        <ToolbarGroup>
          <ToolbarOverflow label={t("panes.profiles.toolbar.more")}>
            <MenubarItem
              disabled={importingFromClipboard}
              onSelect={() => void handleImportFromClipboard()}
            >
              <ClipboardPaste className="size-4" aria-hidden="true" />
              {t("panes.profiles.import.clipboard")}
            </MenubarItem>
            <MenubarItem onSelect={() => setImportOpen(true)}>
              <Upload className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.import")}
            </MenubarItem>
            <MenubarItem onSelect={() => setSubscriptionsOpen(true)}>
              <Rss className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.subscriptions")}
            </MenubarItem>
            <MenubarItem onSelect={() => requestDedupe()}>
              <Filter className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.dedupe")}
            </MenubarItem>
          </ToolbarOverflow>
        </ToolbarGroup>
      </Toolbar>

      {operationError ? <InlinePageError>{operationError}</InlinePageError> : null}
      {profilesQuery.isError ? <InlinePageError>{getErrorMessage(profilesQuery.error)}</InlinePageError> : null}
      {operationMessage ? (
        <div className="border-b bg-connected/10 px-4 py-2 text-sm text-connected">{operationMessage}</div>
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
            {t("panes.profiles.undecodable", { count: undecodableProfiles.toLocaleString() })}
          </span>
        </div>
      ) : null}
    </>
  );
}
