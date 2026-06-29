import { useMemo, useRef, useState } from "react";
import type * as React from "react";
import * as ContextMenu from "@radix-ui/react-context-menu";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getCoreRowModel, useReactTable, type ColumnDef } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  ChevronDown,
  ChevronsDown,
  ChevronsUp,
  ClipboardPaste,
  Clock,
  Columns3,
  Copy,
  Download,
  FileJson2,
  FilePlus2,
  Filter,
  Gauge,
  Inbox,
  Link,
  Pencil,
  Play,
  QrCode,
  Radio,
  RefreshCw,
  RotateCcw,
  Rows3,
  Rss,
  Search,
  Share2,
  Square,
  Trash2,
  Upload,
  Wifi,
  Zap,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { BulkActionBar, Toolbar, ToolbarGroup, ToolbarOverflow } from "@/components/app-shell/toolbar";
import {
  dataTableHeader,
  dataTableRowEven,
  dataTableRowHover,
  dataTableRowOdd,
  dataTableRowSelected,
  dataTableWell,
} from "@/components/app-shell/data-table-surface";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { PageHeader, PageHeaderHeading, PageSection } from "@/components/app-shell/page-section";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { buttonVariants } from "@/components/ui/button-variants";
import { Checkbox } from "@/components/ui/checkbox";
import { EmptyState } from "@/components/ui/empty-state";
import { Input } from "@/components/ui/input";
import {
  Menubar,
  MenubarCheckboxItem,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarSeparator,
  MenubarTrigger,
} from "@/components/ui/menubar";
import { Skeleton } from "@/components/ui/skeleton";
import {
  copyProfiles,
  cancelSpeedtest,
  dedupeProfiles,
  deleteProfiles,
  exportProfileClientConfig,
  exportProfileInnerLinks,
  exportProfileShareLinks,
  exportProfileShareLinksBase64,
  importProfilesFromText,
  listProfiles,
  moveProfile,
  runSpeedtest,
  saveGroupProfile,
  saveProfile,
  setActiveProfile,
  sortProfiles,
  updateSubscriptions,
  useRuntimeEventStore,
  saveTextFile,
} from "@/ipc";
import type {
  ExportProfilesResult,
  ImportProfilesResult,
  ProfileItem_Deserialize,
  ProfileListItem_Serialize,
  ProfileSortKey,
  SpeedActionType,
} from "@/ipc/bindings";
import { useI18n } from "@/i18n/use-i18n";
import { formatDelay, formatSpeed, formatTraffic } from "@/lib/formatting";
import { cn, getErrorMessage } from "@/lib/utils";
import { useModalStore } from "@/stores/modal-store";
import { useProfileColumnsStore } from "@/stores/profile-columns-store";

import { ImportProfilesDialog, SubscriptionsDialog } from "@/features/subscriptions";
import { CONFIG_TYPES, getProtocolLabel, MOVE_ACTIONS, SPEED_ACTIONS } from "./profile-constants";
import { ProfileDialog } from "./profile-dialog";
import { applyLiveUpdates } from "./server-table-live-updates";

type DialogState =
  | { mode: "create"; profile?: null }
  | { mode: "edit"; profile: ProfileListItem_Serialize }
  | null;

type TranslateFn = (key: string, options?: Record<string, unknown>) => string;
type ProfileExportKind = "clientConfig" | "innerLinks" | "shareBase64" | "shareLinks";

type ServerColumn = {
  cell: (item: ProfileListItem_Serialize, rowNumber: number, t: TranslateFn) => React.ReactNode;
  id: string;
  labelKey: string;
  sortKey?: ProfileSortKey;
  width: string;
};

const serverColumns: ServerColumn[] = [
  {
    cell: (item, rowNumber, t) => (
      <span className="flex items-center gap-2">
        {item.isActive ? (
          // The live profile reads as a 6px green dot — intentionally distinct
          // from the blue row-selection state rendered by the surface tokens.
          <span
            aria-label={t("panes.profiles.aria.activeProfile")}
            className="size-1.5 rounded-full bg-connected"
            data-testid="active-profile-marker"
            role="img"
          />
        ) : (
          <span className="size-1.5" aria-hidden="true" />
        )}
        <span className="tabular-nums text-muted-foreground">{rowNumber}</span>
      </span>
    ),
    id: "state",
    labelKey: "panes.profiles.columns.labels.indexHeader",
    width: "5rem",
  },
  {
    cell: (item) => (
      <Badge className="max-w-full justify-start truncate text-muted-foreground" variant="outline">
        <span className="truncate">{getProtocolLabel(item.profile.ConfigType)}</span>
      </Badge>
    ),
    id: "configType",
    labelKey: "panes.profiles.columns.labels.protocol",
    sortKey: "configType",
    width: "8rem",
  },
  {
    cell: (item, _rowNumber, t) => item.profile.Remarks || t("panes.profiles.untitled"),
    id: "remarks",
    labelKey: "panes.profiles.columns.labels.remarks",
    sortKey: "remarks",
    width: "minmax(13rem,1.3fr)",
  },
  {
    cell: (item) => item.profile.Address,
    id: "address",
    labelKey: "panes.profiles.columns.labels.address",
    sortKey: "address",
    width: "minmax(12rem,1fr)",
  },
  {
    cell: (item) => <span className="tabular-nums">{item.profile.Port || ""}</span>,
    id: "port",
    labelKey: "panes.profiles.columns.labels.port",
    sortKey: "port",
    width: "5rem",
  },
  {
    cell: (item) => item.profile.Network || "tcp",
    id: "network",
    labelKey: "panes.profiles.columns.labels.transport",
    sortKey: "network",
    width: "7rem",
  },
  {
    cell: (item) => item.profile.StreamSecurity || "none",
    id: "security",
    labelKey: "panes.profiles.columns.labels.security",
    sortKey: "streamSecurity",
    width: "7rem",
  },
  {
    cell: (item) => formatDelay(item.profileEx.Delay),
    id: "delay",
    labelKey: "panes.profiles.columns.labels.delay",
    sortKey: "delay",
    width: "6rem",
  },
  {
    cell: (item) => formatSpeedOrMessage(item.profileEx.Speed, item.profileEx.Message),
    id: "speed",
    labelKey: "panes.profiles.columns.labels.speed",
    sortKey: "speed",
    width: "7rem",
  },
  {
    cell: (item) => formatTraffic(item.serverStat?.TodayUp),
    id: "todayUp",
    labelKey: "panes.profiles.columns.labels.todayUp",
    width: "8rem",
  },
  {
    cell: (item) => formatTraffic(item.serverStat?.TodayDown),
    id: "todayDown",
    labelKey: "panes.profiles.columns.labels.todayDown",
    width: "8rem",
  },
  {
    cell: (item) => formatTraffic(item.serverStat?.TotalUp),
    id: "totalUp",
    labelKey: "panes.profiles.columns.labels.totalUp",
    width: "8rem",
  },
  {
    cell: (item) => formatTraffic(item.serverStat?.TotalDown),
    id: "totalDown",
    labelKey: "panes.profiles.columns.labels.totalDown",
    width: "8rem",
  },
  {
    cell: (item) => item.profileEx.IpInfo ?? "",
    id: "ipInfo",
    labelKey: "panes.profiles.columns.labels.ipInfo",
    sortKey: "ipInfo",
    width: "10rem",
  },
  {
    cell: (item) => item.profile.Subid,
    id: "subid",
    labelKey: "panes.profiles.columns.labels.group",
    sortKey: "subid",
    width: "8rem",
  },
];

const COLUMN_LABEL_KEY_BY_ID: Record<string, string> = Object.fromEntries(
  serverColumns.map((column) => [column.id, column.labelKey]),
);

// Leading track is the selection checkbox column.
const SELECTION_COLUMN_WIDTH_REM = 2.75;

function buildGridTemplateColumns(columns: ServerColumn[]) {
  return `${SELECTION_COLUMN_WIDTH_REM}rem ${columns.map((column) => column.width).join(" ")}`;
}

function columnMinWidthRem(width: string) {
  // Pick the first rem measurement — the fixed size, or the floor of a minmax().
  const match = /([\d.]+)rem/.exec(width);
  return match ? Number(match[1]) : 8;
}

function buildGridMinWidth(columns: ServerColumn[]) {
  const total = columns.reduce((sum, column) => sum + columnMinWidthRem(column.width), SELECTION_COLUMN_WIDTH_REM);
  return `${total}rem`;
}

export function ProfilesScreen() {
  const [dialogState, setDialogState] = useState<DialogState>(null);
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [filterText, setFilterText] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [importingFromClipboard, setImportingFromClipboard] = useState(false);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [operationMessage, setOperationMessage] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<string[] | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [sortState, setSortState] = useState<{ ascending: boolean; key: ProfileSortKey } | null>(null);
  const [subscriptionsOpen, setSubscriptionsOpen] = useState(false);
  const { t } = useI18n();
  const columnVisibility = useProfileColumnsStore((state) => state.columnVisibility);
  const setColumnVisibility = useProfileColumnsStore((state) => state.setColumnVisibility);
  const resetColumnVisibility = useProfileColumnsStore((state) => state.resetColumnVisibility);
  const serverStatsByProfileId = useRuntimeEventStore((state) => state.serverStatsByProfileId);
  const speedtestResultsByProfileId = useRuntimeEventStore((state) => state.speedtestResultsByProfileId);
  const speedtestRunning = useRuntimeEventStore((state) => state.speedtestRunning);
  const setSpeedtestRunning = useRuntimeEventStore((state) => state.setSpeedtestRunning);
  const openModal = useModalStore((state) => state.openModal);
  const queryClient = useQueryClient();
  const filter = filterText.trim();
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, filter || null),
    queryKey: profilesQueryKey(filter),
  });
  const profiles = useMemo(
    () => applyLiveUpdates(profilesQuery.data ?? [], serverStatsByProfileId, speedtestResultsByProfileId),
    [profilesQuery.data, serverStatsByProfileId, speedtestResultsByProfileId],
  );

  const tableColumns = useMemo<ColumnDef<ProfileListItem_Serialize>[]>(
    () =>
      serverColumns.map((column) => ({
        id: column.id,
        header: column.labelKey,
        // The structural `#`/state column is always shown; everything else can
        // be collapsed through the column menu.
        enableHiding: column.id !== "state",
      })),
    [],
  );
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Table owns stable row-model helpers internally.
  const table = useReactTable({
    columns: tableColumns,
    data: profiles,
    getCoreRowModel: getCoreRowModel(),
    getRowId: (row) => row.profile.IndexId,
    onColumnVisibilityChange: setColumnVisibility,
    state: { columnVisibility },
  });
  const hideableColumns = table.getAllLeafColumns().filter((column) => column.getCanHide());
  const visibleColumns = useMemo(
    () => serverColumns.filter((column) => column.id === "state" || columnVisibility[column.id] !== false),
    [columnVisibility],
  );
  const gridTemplateColumns = useMemo(() => buildGridTemplateColumns(visibleColumns), [visibleColumns]);
  const gridMinWidth = useMemo(() => buildGridMinWidth(visibleColumns), [visibleColumns]);
  const rows = table.getRowModel().rows;
  const viewportRef = useRef<HTMLDivElement>(null);
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    estimateSize: () => 38,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 520, width: 1200 },
    overscan: 10,
  });
  const visibleRows = rowVirtualizer.getVirtualItems();
  const renderedRows =
    visibleRows.length > 0
      ? visibleRows
      : rows.slice(0, Math.min(rows.length, 30)).map((row, index) => ({
          index,
          key: row.id,
          start: index * 38,
        }));
  const selected = profiles.filter((item) => selectedIds.has(item.profile.IndexId));
  const primarySelection = selected[0] ?? null;
  const allVisibleSelected = profiles.length > 0 && profiles.every((item) => selectedIds.has(item.profile.IndexId));
  const someVisibleSelected = profiles.some((item) => selectedIds.has(item.profile.IndexId));
  const allVisibleCheckboxState = allVisibleSelected ? true : someVisibleSelected ? "indeterminate" : false;

  async function runOperation(operation: () => Promise<unknown>) {
    setOperationError(null);
    setOperationMessage(null);
    try {
      await operation();
      await queryClient.invalidateQueries({ queryKey: ["profiles"] });
    } catch (error) {
      setOperationError(getErrorMessage(error));
    }
  }

  // Destructive: route deletions through a confirmation gate instead of firing
  // the IPC call directly from the trigger.
  function requestDelete(indexIds: string[]) {
    if (indexIds.length > 0) {
      setPendingDelete(indexIds);
    }
  }

  function confirmDelete() {
    const indexIds = pendingDelete;
    setPendingDelete(null);
    if (indexIds && indexIds.length > 0) {
      void runOperation(() => deleteProfiles(indexIds));
    }
  }

  function toggleSelection(indexId: string, selected: boolean) {
    setSelectedIds((current) => {
      const next = new Set(current);

      if (selected) {
        next.add(indexId);
      } else {
        next.delete(indexId);
      }

      return next;
    });
  }

  function selectOnly(indexId: string) {
    setSelectedIds(new Set([indexId]));
  }

  function toggleAllVisible(selected: boolean) {
    setSelectedIds(selected ? new Set(profiles.map((item) => item.profile.IndexId)) : new Set());
  }

  async function handleSort(sortKey: ProfileSortKey) {
    const ascending = sortState?.key === sortKey ? !sortState.ascending : true;
    setSortState({ ascending, key: sortKey });
    await runOperation(() => sortProfiles(null, sortKey, ascending));
  }

  async function handleSave(profile: ProfileItem_Deserialize) {
    const save = profile.ConfigType === CONFIG_TYPES.PolicyGroup || profile.ConfigType === CONFIG_TYPES.ProxyChain
      ? saveGroupProfile
      : saveProfile;
    await runOperation(() => save(profile));
    setDialogState(null);
  }

  async function handleImportFromClipboard() {
    setOperationError(null);
    setOperationMessage(null);

    if (!navigator.clipboard?.readText) {
      setOperationError(t("panes.profiles.import.clipboardUnavailable"));
      return;
    }

    setImportingFromClipboard(true);
    try {
      const text = (await navigator.clipboard.readText()).trim();
      if (!text) {
        throw new Error(t("panes.profiles.import.clipboardEmpty"));
      }

      const result = await importProfilesFromText(text, null, false);
      await handleDialogImport(result);
    } catch (error) {
      setOperationError(getErrorMessage(error));
    } finally {
      setImportingFromClipboard(false);
    }
  }

  async function handleDialogImport(result: ImportProfilesResult) {
    setOperationError(null);
    setOperationMessage(formatImportOperationMessage(result));
    const importedIndexIds = result.importedIndexIds ?? [];
    if (importedIndexIds.length > 0) {
      setFilterText("");
      setSelectedIds(new Set(importedIndexIds));
      const refreshedProfiles = await listProfiles(null, null);
      queryClient.setQueryData(profilesQueryKey(""), refreshedProfiles);
    } else {
      await queryClient.invalidateQueries({ queryKey: ["profiles"] });
    }
    await queryClient.invalidateQueries({ queryKey: ["subscriptions"] });
  }

  const selectedIdsArray = selected.map((item) => item.profile.IndexId);

  async function handleExport(kind: ProfileExportKind, indexIds = selectedIdsArray, showQr = false, saveFile = false) {
    setOperationError(null);
    setOperationMessage(null);
    if (indexIds.length === 0) {
      setOperationError(t("panes.profiles.export.noSelection"));
      return;
    }

    try {
      const result = await runProfileExport(kind, indexIds);
      if (showQr) {
        openModal("qr", { qrContent: result.text });
        return;
      }

      if (saveFile) {
        const path = await saveTextFile({
          defaultPath: exportFileName(kind),
          filters: [exportFileFilter(kind)],
          text: result.text,
        });
        if (path) {
          setOperationMessage(t("panes.profiles.export.savedFile", { path }));
        }
        return;
      }

      if (!navigator.clipboard?.writeText) {
        throw new Error(t("panes.profiles.export.clipboardUnavailable"));
      }
      await navigator.clipboard.writeText(result.text);
      setOperationMessage(t("panes.profiles.export.copied", { count: result.count }));
    } catch (error) {
      setOperationError(getErrorMessage(error));
    }
  }

  async function handleSpeedtest(action: SpeedActionType, indexIds = selectedIdsArray) {
    setColumnVisibility((current) => ({ ...current, delay: true, speed: true }));
    setSpeedtestRunning(true);
    try {
      await runOperation(() => runSpeedtest(action, indexIds));
    } finally {
      setSpeedtestRunning(false);
    }
  }

  async function handleCancelSpeedtest() {
    await runOperation(() => cancelSpeedtest());
    setSpeedtestRunning(false);
  }

  return (
    <PageSection aria-label={t("panes.profiles.title")}>
      <PageHeader>
        <PageHeaderHeading
          count={t("panes.profiles.toolbar.rows", { rows: profiles.length.toLocaleString() })}
          icon={Rows3}
          title={t("panes.profiles.title")}
        />

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
        </ToolbarGroup>

        <ToolbarGroup>
          <SpeedtestSplitButton
            disabled={profiles.length === 0}
            onCancel={handleCancelSpeedtest}
            onRun={handleSpeedtest}
            running={speedtestRunning}
          />
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
            <MenubarItem onSelect={() => void runOperation(() => updateSubscriptions(null, false, null))}>
              <RefreshCw className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.updateSubs")}
            </MenubarItem>
            <MenubarSeparator />
            <MenubarItem onSelect={() => void runOperation(() => dedupeProfiles(null, null))}>
              <Filter className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.dedupe")}
            </MenubarItem>
            <MenubarSeparator />
            <ExportMenuItems
              onExport={(kind) => void handleExport(kind)}
              onSave={(kind) => void handleExport(kind, selectedIdsArray, false, true)}
              onShowQr={() => void handleExport("shareLinks", selectedIdsArray, true)}
              t={t}
            />
          </ToolbarOverflow>
        </ToolbarGroup>
      </Toolbar>

      {selected.length > 0 ? (
        <BulkActionBar>
          <span className="text-sm font-medium">
            {t("panes.profiles.bulk.selected", { count: selected.length })}
          </span>
          <div className="ms-auto flex items-center gap-2">
            <Button
              onClick={() =>
                primarySelection && void runOperation(() => setActiveProfile(primarySelection.profile.IndexId))
              }
              size="sm"
              type="button"
              variant="outline"
            >
              <Play className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.activate")}
            </Button>
            <Button
              onClick={() => primarySelection && setDialogState({ mode: "edit", profile: primarySelection })}
              size="sm"
              type="button"
              variant="outline"
            >
              <Pencil className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.edit")}
            </Button>
            <Button
              onClick={() => void runOperation(() => copyProfiles(selectedIdsArray))}
              size="sm"
              type="button"
              variant="outline"
            >
              <Copy className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.copy")}
            </Button>
            <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
              <MenubarMenu>
                <MenubarTrigger asChild>
                  <Button size="sm" type="button" variant="outline">
                    <Share2 className="size-4" aria-hidden="true" />
                    {t("panes.profiles.export.export")}
                  </Button>
                </MenubarTrigger>
                <MenubarContent align="end">
                  <ExportMenuItems
                    onExport={(kind) => void handleExport(kind, selectedIdsArray)}
                    onSave={(kind) => void handleExport(kind, selectedIdsArray, false, true)}
                    onShowQr={() => void handleExport("shareLinks", selectedIdsArray, true)}
                    t={t}
                  />
                </MenubarContent>
              </MenubarMenu>
            </Menubar>
            <Button
              onClick={() => requestDelete(selectedIdsArray)}
              size="sm"
              type="button"
              variant="outline"
            >
              <Trash2 className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.delete")}
            </Button>
          </div>
        </BulkActionBar>
      ) : null}

      {operationError ? <InlinePageError>{operationError}</InlinePageError> : null}
      {profilesQuery.isError ? <InlinePageError>{getErrorMessage(profilesQuery.error)}</InlinePageError> : null}
      {operationMessage ? (
        <div className="border-b bg-connected/10 px-4 py-2 text-sm text-connected">{operationMessage}</div>
      ) : null}

      <div className="min-h-0 flex-1 overflow-hidden p-4">
        <div
          aria-busy={profilesQuery.isLoading}
          aria-colcount={visibleColumns.length + 1}
          aria-label={t("panes.profiles.title")}
          aria-rowcount={profiles.length}
          className={cn("flex h-full min-h-[18rem] flex-col", dataTableWell)}
          role="table"
        >
          <div className="overflow-x-auto border-b">
            <div
              aria-rowindex={1}
              className={cn("grid items-center", dataTableHeader)}
              role="row"
              style={{ gridTemplateColumns, minWidth: gridMinWidth }}
            >
              <div
                aria-colindex={1}
                className="flex h-9 items-center justify-center border-e px-2"
                role="columnheader"
              >
                <Checkbox
                  aria-label={t("panes.profiles.aria.selectAll")}
                  checked={allVisibleCheckboxState}
                  onCheckedChange={(checked) => toggleAllVisible(checked === true)}
                />
              </div>
              {visibleColumns.map((column, columnIndex) => (
                <div
                  aria-colindex={columnIndex + 2}
                  aria-sort={sortAriaValue(column, sortState)}
                  className="flex h-9 min-w-0 items-center border-e px-2 last:border-e-0"
                  key={column.id}
                  role="columnheader"
                >
                  {column.sortKey ? (
                    <button
                      className="flex min-w-0 items-center gap-1 text-start"
                      onClick={() => void handleSort(column.sortKey!)}
                      type="button"
                    >
                      <span className="truncate">{t(column.labelKey)}</span>
                      {sortState?.key === column.sortKey ? (
                        sortState.ascending ? (
                          <ArrowUp className="size-3" aria-hidden="true" />
                        ) : (
                          <ArrowDown className="size-3" aria-hidden="true" />
                        )
                      ) : null}
                    </button>
                  ) : (
                    <span className="truncate">{t(column.labelKey)}</span>
                  )}
                </div>
              ))}
            </div>
          </div>

          <div
            className="min-h-0 flex-1 overflow-auto"
            data-testid="server-table-viewport"
            ref={viewportRef}
            role="rowgroup"
          >
            {profilesQuery.isLoading ? (
              <ProfileSkeletonRows
                aria-label={t("panes.profiles.loading")}
                columnCount={visibleColumns.length}
                gridMinWidth={gridMinWidth}
                gridTemplateColumns={gridTemplateColumns}
              />
            ) : rows.length === 0 ? (
              <EmptyState
                className="h-full content-center"
                description={t("panes.profiles.emptyDescription")}
                icon={Inbox}
                title={t("panes.profiles.empty")}
              />
            ) : (
              <div className="relative" style={{ height: rowVirtualizer.getTotalSize(), minWidth: gridMinWidth }}>
                {renderedRows.map((virtualRow) => {
                  const row = rows[virtualRow.index];
                  if (!row) {
                    return null;
                  }

                  const item = row.original;
                  const indexId = item.profile.IndexId;
                  const isSelected = selectedIds.has(indexId);

                  return (
                    <ProfileRowContextMenu
                      item={item}
                      key={row.id}
                      onActivate={() => void runOperation(() => setActiveProfile(indexId))}
                      onCopy={() => void runOperation(() => copyProfiles(selectedIds.has(indexId) ? selectedIdsArray : [indexId]))}
                      onDelete={() => requestDelete(selectedIds.has(indexId) ? selectedIdsArray : [indexId])}
                      onEdit={() => setDialogState({ mode: "edit", profile: item })}
                      onExport={(kind) =>
                        void handleExport(kind, selectedIds.has(indexId) ? selectedIdsArray : [indexId])
                      }
                      onSave={(kind) =>
                        void handleExport(kind, selectedIds.has(indexId) ? selectedIdsArray : [indexId], false, true)
                      }
                      onMove={(action) => void runOperation(() => moveProfile(null, indexId, action, null))}
                      onSelectOnly={() => selectOnly(indexId)}
                      onShowQr={() =>
                        void handleExport("shareLinks", selectedIds.has(indexId) ? selectedIdsArray : [indexId], true)
                      }
                    >
                      <div
                        aria-selected={isSelected}
                        className={cn(
                          "absolute start-0 grid h-9.5 items-center border-b text-sm outline-none",
                          isSelected
                            ? dataTableRowSelected
                            : cn(
                                virtualRow.index % 2 === 0 ? dataTableRowEven : dataTableRowOdd,
                                dataTableRowHover,
                              ),
                        )}
                        data-testid="server-row"
                        draggable
                        onClick={(event) => {
                          if (event.metaKey || event.ctrlKey) {
                            toggleSelection(indexId, !isSelected);
                          } else {
                            selectOnly(indexId);
                          }
                        }}
                        onDoubleClick={() => void runOperation(() => setActiveProfile(indexId))}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            event.preventDefault();
                            void runOperation(() => setActiveProfile(indexId));
                          }
                          if (event.key === " ") {
                            event.preventDefault();
                            toggleSelection(indexId, !isSelected);
                          }
                        }}
                        onDragOver={(event) => event.preventDefault()}
                        onDragStart={(event) => {
                          setDraggedId(indexId);
                          event.dataTransfer.effectAllowed = "move";
                          event.dataTransfer.setData("text/profile-id", indexId);
                        }}
                        onDrop={(event) => {
                          event.preventDefault();
                          const sourceId = event.dataTransfer.getData("text/profile-id") || draggedId;
                          if (sourceId && sourceId !== indexId) {
                            void runOperation(() => moveProfile(null, sourceId, MOVE_ACTIONS.Position, virtualRow.index));
                          }
                          setDraggedId(null);
                        }}
                        aria-rowindex={virtualRow.index + 2}
                        role="row"
                        style={{
                          gridTemplateColumns,
                          minWidth: gridMinWidth,
                          transform: `translateY(${virtualRow.start}px)`,
                        }}
                        tabIndex={0}
                      >
                        <div
                          aria-colindex={1}
                          className="flex h-full items-center justify-center border-e px-2"
                          role="cell"
                        >
                          <Checkbox
                            aria-label={t("panes.profiles.aria.selectRow", { name: item.profile.Remarks || indexId })}
                            checked={isSelected}
                            onClick={(event) => event.stopPropagation()}
                            onCheckedChange={(checked) => toggleSelection(indexId, checked === true)}
                          />
                        </div>
                        {visibleColumns.map((column, columnIndex) => {
                          const cell = column.cell(item, virtualRow.index + 1, t);

                          return (
                            <div
                              aria-colindex={columnIndex + 2}
                              className="flex h-full min-w-0 items-center border-e px-2 last:border-e-0"
                              key={column.id}
                              role="cell"
                              title={cellTitle(cell)}
                            >
                              <span className="truncate">{cell}</span>
                            </div>
                          );
                        })}
                      </div>
                    </ProfileRowContextMenu>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </div>

      <ProfileDialog
        mode={dialogState?.mode ?? "create"}
        onOpenChange={(open) => !open && setDialogState(null)}
        onSubmit={handleSave}
        open={Boolean(dialogState)}
        profile={dialogState?.mode === "edit" ? dialogState.profile : null}
      />
      <ImportProfilesDialog
        onImported={handleDialogImport}
        onOpenChange={setImportOpen}
        open={importOpen}
      />
      <SubscriptionsDialog
        onChanged={() => void queryClient.invalidateQueries({ queryKey: ["profiles"] })}
        onOpenChange={setSubscriptionsOpen}
        open={subscriptionsOpen}
      />
      <AlertDialog open={pendingDelete !== null} onOpenChange={(open) => !open && setPendingDelete(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("confirm.deleteProfilesTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("confirm.deleteProfilesDescription", { count: pendingDelete?.length ?? 0 })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction className={buttonVariants({ variant: "destructive" })} onClick={confirmDelete}>
              {t("confirm.deleteProfilesConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </PageSection>
  );
}

// Speedtest split button: the default `Fast` ping runs straight from the primary
// control, while the chevron opens a menu for the remaining probe modes plus the
// running-only Stop. The dropdown reuses the Menubar primitive (no new
// dependency) so its trigger and items expose `menuitem` roles, mirroring the
// Columns menu.
function SpeedtestSplitButton({
  disabled,
  onCancel,
  onRun,
  running,
}: {
  disabled: boolean;
  onCancel: () => Promise<void>;
  onRun: (action: SpeedActionType) => Promise<void>;
  running: boolean;
}) {
  const { t } = useI18n();

  return (
    <div className="flex items-center">
      <Button
        className="rounded-e-none"
        disabled={disabled || running}
        onClick={() => void onRun(SPEED_ACTIONS.FastRealping)}
        size="sm"
        title={t("panes.profiles.speedtest.buttonTitle", { label: t("panes.profiles.speedtest.fast") })}
        type="button"
        variant="outline"
      >
        <Zap className="size-4" aria-hidden="true" />
        {t("panes.profiles.speedtest.fast")}
      </Button>
      <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
        <MenubarMenu>
          <MenubarTrigger asChild>
            <Button
              aria-label={t("panes.profiles.speedtest.more")}
              className="rounded-s-none border-s-0 px-2"
              disabled={disabled}
              size="sm"
              title={t("panes.profiles.speedtest.more")}
              type="button"
              variant="outline"
            >
              <ChevronDown className="size-4" aria-hidden="true" />
            </Button>
          </MenubarTrigger>
          <MenubarContent align="start">
            <SpeedMenuItem
              action={SPEED_ACTIONS.Tcping}
              disabled={running}
              icon={Activity}
              label={t("panes.profiles.speedtest.tcp")}
              onRun={onRun}
            />
            <SpeedMenuItem
              action={SPEED_ACTIONS.Realping}
              disabled={running}
              icon={Clock}
              label={t("panes.profiles.speedtest.real")}
              onRun={onRun}
            />
            <SpeedMenuItem
              action={SPEED_ACTIONS.UdpTest}
              disabled={running}
              icon={Radio}
              label={t("panes.profiles.speedtest.udp")}
              onRun={onRun}
            />
            <SpeedMenuItem
              action={SPEED_ACTIONS.Speedtest}
              disabled={running}
              icon={Gauge}
              label={t("panes.profiles.speedtest.speed")}
              onRun={onRun}
            />
            <SpeedMenuItem
              action={SPEED_ACTIONS.Mixedtest}
              disabled={running}
              icon={Wifi}
              label={t("panes.profiles.speedtest.mixed")}
              onRun={onRun}
            />
            <MenubarSeparator />
            <MenubarItem
              disabled={!running}
              onSelect={() => void onCancel()}
              title={t("panes.profiles.speedtest.cancelTitle")}
            >
              <Square className="size-4" aria-hidden="true" />
              {t("panes.profiles.speedtest.stop")}
            </MenubarItem>
          </MenubarContent>
        </MenubarMenu>
      </Menubar>
    </div>
  );
}

function SpeedMenuItem({
  action,
  disabled,
  icon: Icon,
  label,
  onRun,
}: {
  action: SpeedActionType;
  disabled: boolean;
  icon: LucideIcon;
  label: string;
  onRun: (action: SpeedActionType) => Promise<void>;
}) {
  return (
    <MenubarItem disabled={disabled} onSelect={() => void onRun(action)}>
      <Icon className="size-4" aria-hidden="true" />
      {label}
    </MenubarItem>
  );
}

function ExportMenuItems({
  onExport,
  onSave,
  onShowQr,
  t,
}: {
  onExport: (kind: ProfileExportKind) => void;
  onSave: (kind: ProfileExportKind) => void;
  onShowQr: () => void;
  t: TranslateFn;
}) {
  return (
    <>
      <MenubarItem onSelect={() => onExport("shareLinks")}>
        <Link className="size-4" aria-hidden="true" />
        {t("panes.profiles.export.shareLinks")}
      </MenubarItem>
      <MenubarItem onSelect={() => onExport("shareBase64")}>
        <Share2 className="size-4" aria-hidden="true" />
        {t("panes.profiles.export.shareBase64")}
      </MenubarItem>
      <MenubarItem onSelect={() => onExport("innerLinks")}>
        <Link className="size-4" aria-hidden="true" />
        {t("panes.profiles.export.innerLinks")}
      </MenubarItem>
      <MenubarItem onSelect={() => onExport("clientConfig")}>
        <FileJson2 className="size-4" aria-hidden="true" />
        {t("panes.profiles.export.clientConfig")}
      </MenubarItem>
      <MenubarSeparator />
      <MenubarItem onSelect={onShowQr}>
        <QrCode className="size-4" aria-hidden="true" />
        {t("panes.profiles.export.showQr")}
      </MenubarItem>
      <MenubarItem onSelect={() => onSave("shareLinks")}>
        <Download className="size-4" aria-hidden="true" />
        {t("panes.profiles.export.saveShareLinks")}
      </MenubarItem>
      <MenubarItem onSelect={() => onSave("clientConfig")}>
        <FileJson2 className="size-4" aria-hidden="true" />
        {t("panes.profiles.export.saveClientConfig")}
      </MenubarItem>
    </>
  );
}

function runProfileExport(kind: ProfileExportKind, indexIds: string[]): Promise<ExportProfilesResult> {
  switch (kind) {
    case "clientConfig":
      return exportProfileClientConfig(indexIds);
    case "innerLinks":
      return exportProfileInnerLinks(indexIds);
    case "shareBase64":
      return exportProfileShareLinksBase64(indexIds);
    case "shareLinks":
      return exportProfileShareLinks(indexIds);
  }
}

function exportFileName(kind: ProfileExportKind) {
  switch (kind) {
    case "clientConfig":
      return "voyavpn-client-config.json";
    case "innerLinks":
      return "voyavpn-inner-links.txt";
    case "shareBase64":
      return "voyavpn-share-links-base64.txt";
    case "shareLinks":
      return "voyavpn-share-links.txt";
  }
}

function exportFileFilter(kind: ProfileExportKind) {
  return kind === "clientConfig"
    ? { extensions: ["json"], name: "JSON" }
    : { extensions: ["txt"], name: "Text" };
}

// Mirror the grid geometry of a real row so the loading state holds the same
// layout as the populated table — the connections pane skeleton pattern.
function ProfileSkeletonRows({
  columnCount,
  gridMinWidth,
  gridTemplateColumns,
  ...props
}: React.ComponentProps<"div"> & {
  columnCount: number;
  gridMinWidth: string;
  gridTemplateColumns: string;
}) {
  return (
    <div role="status" {...props}>
      {Array.from({ length: 12 }).map((_, rowIndex) => (
        <div
          className="grid h-9.5 items-center border-b"
          key={rowIndex}
          style={{ gridTemplateColumns, minWidth: gridMinWidth }}
        >
          <div className="flex h-full items-center justify-center border-e px-2">
            <Skeleton className="size-4 rounded-sm" />
          </div>
          {Array.from({ length: columnCount }).map((_, columnIndex) => (
            <div className="flex h-full items-center border-e px-2 last:border-e-0" key={columnIndex}>
              <Skeleton className="h-4 w-3/4" />
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function ProfileRowContextMenu({
  children,
  item,
  onActivate,
  onCopy,
  onDelete,
  onEdit,
  onExport,
  onMove,
  onSave,
  onSelectOnly,
  onShowQr,
}: {
  children: React.ReactNode;
  item: ProfileListItem_Serialize;
  onActivate: () => void;
  onCopy: () => void;
  onDelete: () => void;
  onEdit: () => void;
  onExport: (kind: ProfileExportKind) => void;
  onMove: (action: number) => void;
  onSave: (kind: ProfileExportKind) => void;
  onSelectOnly: () => void;
  onShowQr: () => void;
}) {
  const { t } = useI18n();

  return (
    <ContextMenu.Root onOpenChange={(open) => open && onSelectOnly()}>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="z-50 min-w-48 rounded-md border bg-card p-1 text-sm shadow-xl outline-none">
          <ContextMenu.Label className="truncate px-2 py-1.5 text-xs text-muted-foreground">
            {item.profile.Remarks || t("panes.profiles.untitled")}
          </ContextMenu.Label>
          <ContextItem icon={Play} label={t("panes.profiles.menu.activate")} onSelect={onActivate} />
          <ContextItem icon={Pencil} label={t("panes.profiles.menu.edit")} onSelect={onEdit} />
          <ContextItem icon={Copy} label={t("panes.profiles.menu.copy")} onSelect={onCopy} />
          <ContextItem icon={Trash2} label={t("panes.profiles.menu.delete")} onSelect={onDelete} />
          <ContextMenu.Separator className="my-1 h-px bg-border" />
          <ContextItem icon={Link} label={t("panes.profiles.export.shareLinks")} onSelect={() => onExport("shareLinks")} />
          <ContextItem icon={Share2} label={t("panes.profiles.export.shareBase64")} onSelect={() => onExport("shareBase64")} />
          <ContextItem icon={Link} label={t("panes.profiles.export.innerLinks")} onSelect={() => onExport("innerLinks")} />
          <ContextItem icon={FileJson2} label={t("panes.profiles.export.clientConfig")} onSelect={() => onExport("clientConfig")} />
          <ContextItem icon={QrCode} label={t("panes.profiles.export.showQr")} onSelect={onShowQr} />
          <ContextItem icon={Download} label={t("panes.profiles.export.saveShareLinks")} onSelect={() => onSave("shareLinks")} />
          <ContextItem icon={FileJson2} label={t("panes.profiles.export.saveClientConfig")} onSelect={() => onSave("clientConfig")} />
          <ContextMenu.Separator className="my-1 h-px bg-border" />
          <ContextItem icon={ChevronsUp} label={t("panes.profiles.menu.moveTop")} onSelect={() => onMove(MOVE_ACTIONS.Top)} />
          <ContextItem icon={ArrowUp} label={t("panes.profiles.menu.moveUp")} onSelect={() => onMove(MOVE_ACTIONS.Up)} />
          <ContextItem icon={ArrowDown} label={t("panes.profiles.menu.moveDown")} onSelect={() => onMove(MOVE_ACTIONS.Down)} />
          <ContextItem icon={ChevronsDown} label={t("panes.profiles.menu.moveBottom")} onSelect={() => onMove(MOVE_ACTIONS.Bottom)} />
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

function ContextItem({
  icon: Icon,
  label,
  onSelect,
}: {
  icon: LucideIcon;
  label: string;
  onSelect: () => void;
}) {
  return (
    <ContextMenu.Item
      className="flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 outline-none focus:bg-accent focus:text-accent-foreground"
      onSelect={onSelect}
    >
      <Icon className="size-4" aria-hidden="true" />
      {label}
    </ContextMenu.Item>
  );
}

function profilesQueryKey(filter: string) {
  return ["profiles", { filter }] as const;
}

function formatImportOperationMessage(result: ImportProfilesResult) {
  const imported = result.imported ?? 0;
  const updated = result.updated ?? 0;
  const skipped = result.skipped ?? 0;
  const failed = result.failed ?? 0;
  const filtered = result.filtered ?? 0;
  const deduped = result.deduped ?? 0;
  const removedDuplicates = result.removedDuplicates ?? 0;
  const parts = [`Imported ${imported.toLocaleString()} profile${imported === 1 ? "" : "s"}.`];

  if (updated > 0) {
    parts.push(`${updated.toLocaleString()} updated.`);
  }
  if (removedDuplicates > 0) {
    parts.push(
      `${removedDuplicates.toLocaleString()} duplicate${removedDuplicates === 1 ? "" : "s"} removed.`,
    );
  }
  if (skipped > 0) {
    parts.push(`${skipped.toLocaleString()} skipped.`);
  }
  if (failed > 0) {
    parts.push(`${failed.toLocaleString()} failed to parse.`);
  }
  if (filtered > 0) {
    parts.push(`${filtered.toLocaleString()} filtered.`);
  }
  if (deduped > 0) {
    parts.push(`${deduped.toLocaleString()} duplicate${deduped === 1 ? "" : "s"} skipped from payload.`);
  }

  return parts.join(" ");
}

function sortAriaValue(
  column: ServerColumn,
  sortState: { ascending: boolean; key: ProfileSortKey } | null,
) {
  if (!column.sortKey || sortState?.key !== column.sortKey) {
    return "none" as const;
  }

  return sortState.ascending ? "ascending" : "descending";
}

function cellTitle(cell: React.ReactNode) {
  return typeof cell === "string" || typeof cell === "number" ? String(cell) : undefined;
}

function formatSpeedOrMessage(speed: number | null, message?: string | null) {
  if (isSpeedtestStatusMessage(message)) {
    return message;
  }

  const speedLabel = formatSpeed(speed);

  if (speedLabel) {
    return speedLabel;
  }

  if (!message || /^-?\d+(\.\d+)?$/.test(message)) {
    return "";
  }

  return message;
}

function isSpeedtestStatusMessage(message?: string | null) {
  return Boolean(message && !/^-?\d+(\.\d+)?$/.test(message));
}
