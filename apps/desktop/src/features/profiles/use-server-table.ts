import { useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getCoreRowModel, useReactTable, type ColumnDef } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";

import {
  cancelSpeedtest,
  dedupeProfiles,
  deleteProfiles,
  importProfilesFromText,
  listProfiles,
  runSpeedtest,
  saveGroupProfile,
  saveProfile,
  saveTextFile,
  sortProfiles,
  useRuntimeEventStore,
} from "@/ipc";
import type {
  ImportProfilesResult,
  Profile,
  ProfileListEntry,
  ProfileSortKey,
  ServerStatItem,
  SpeedtestKind,
  SpeedtestResult,
  SpeedtestTarget,
} from "@/ipc/bindings";
import { profilesQueryKey } from "@/ipc/query-keys";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { useProfileColumnsStore } from "@/stores/profile-columns-store";

import {
  exportFileFilter,
  exportFileName,
  formatImportSummary,
  isShareLinkExport,
  runProfileExport,
  supportsShareLinkExport,
  type ProfileExportKind,
} from "./server-table-actions";
import {
  buildGridMinWidth,
  buildGridTemplateColumns,
  serverColumns,
} from "./server-table-columns";
import { CONFIG_TYPES } from "./profile-constants";
import { applyLiveUpdates } from "./server-table-live-updates";

// The statistics stream ticks once per second while traffic flows and the
// speedtest stream bursts one pending marker per selected profile, so the live
// maps are only subscribed to while a column that can actually show them is
// visible. Subscribing unconditionally re-rendered the whole virtualized table
// (and rebuilt the TanStack row model) once per second to update cells that are
// hidden by default.
const TRAFFIC_COLUMN_IDS = ["todayUp", "todayDown", "totalUp", "totalDown"];
const METRIC_COLUMN_IDS = ["delay", "speed", "ipInfo"];
const EMPTY_SERVER_STATS: Record<string, ServerStatItem> = {};
const EMPTY_SPEEDTEST_RESULTS: Record<string, SpeedtestResult> = {};

type DialogState =
  | { mode: "create"; profile?: null }
  | { mode: "edit"; profile: ProfileListEntry }
  | null;

export function useServerTable() {
  const [dialogState, setDialogStateInternal] = useState<DialogState>(null);
  const [filterText, setFilterText] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [importingFromClipboard, setImportingFromClipboard] = useState(false);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [operationMessage, setOperationMessage] = useState<string | null>(null);
  const [pendingDedupe, setPendingDedupe] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<string[] | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [shareQrContent, setShareQrContent] = useState<string | null>(null);
  const [sortState, setSortState] = useState<{ ascending: boolean; key: ProfileSortKey } | null>(null);
  const [subscriptionsOpen, setSubscriptionsOpen] = useState(false);
  const { t } = useI18n();
  const columnVisibility = useProfileColumnsStore((state) => state.columnVisibility);
  const setColumnVisibility = useProfileColumnsStore((state) => state.setColumnVisibility);
  const resetColumnVisibility = useProfileColumnsStore((state) => state.resetColumnVisibility);
  const trafficColumnsVisible = TRAFFIC_COLUMN_IDS.some((id) => columnVisibility[id] !== false);
  const metricColumnsVisible = METRIC_COLUMN_IDS.some((id) => columnVisibility[id] !== false);
  const serverStatsByProfileId = useRuntimeEventStore((state) =>
    trafficColumnsVisible ? state.serverStatsByProfileId : EMPTY_SERVER_STATS,
  );
  const speedtestResultsByProfileId = useRuntimeEventStore((state) =>
    metricColumnsVisible ? state.speedtestResultsByProfileId : EMPTY_SPEEDTEST_RESULTS,
  );
  const speedtestRunning = useRuntimeEventStore((state) => state.speedtestRunning);
  const setSpeedtestRunning = useRuntimeEventStore((state) => state.setSpeedtestRunning);
  const queryClient = useQueryClient();
  const filter = filterText.trim();
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, filter || null),
    queryKey: profilesQueryKey(filter),
  });
  const profiles = useMemo(
    () =>
      applyLiveUpdates(
        profilesQuery.data?.entries ?? [],
        serverStatsByProfileId,
        speedtestResultsByProfileId,
      ),
    [profilesQuery.data, serverStatsByProfileId, speedtestResultsByProfileId],
  );
  // Stored servers this build could not read. The backend skips those rows so
  // one of them cannot hide every other server, and reports how many it
  // skipped; the toolbar states it, because a list that is quietly short is
  // indistinguishable from data loss.
  const undecodableProfiles = profilesQuery.data?.undecodableProfiles ?? 0;

  const tableColumns = useMemo<ColumnDef<ProfileListEntry>[]>(
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
    getRowId: (row) => row.profile.id,
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
  // Reports whether the operation succeeded so callers that own a dialog can
  // keep it open (with the user's edits) when the backend rejects the request.
  // `onError` redirects the message to that dialog instead of the toolbar
  // banner, which a modal would cover.
  async function runOperation(
    operation: () => Promise<unknown>,
    onError: (message: string) => void = setOperationError,
  ) {
    setOperationError(null);
    setOperationMessage(null);
    try {
      // Every command routed through here emits its own profiles invalidation.
      await operation();
      return true;
    } catch (error) {
      onError(getErrorMessage(error));
      return false;
    }
  }

  // Opening or closing the editor drops the previous rejection message.
  function setDialogState(next: DialogState) {
    setSaveError(null);
    setDialogStateInternal(next);
  }

  // Destructive: route deletions through a confirmation gate instead of firing
  // the IPC call directly from the trigger.
  function requestDelete(indexIds: string[]) {
    if (indexIds.length > 0) {
      setPendingDelete(indexIds);
    }
  }

  // Destructive: dedupe deletes every duplicate across all subscriptions, so it
  // goes through the same confirmation gate as delete instead of firing from a
  // single menu click.
  function requestDedupe() {
    setPendingDedupe(true);
  }

  async function confirmDedupe() {
    setPendingDedupe(false);
    setOperationError(null);
    setOperationMessage(null);
    try {
      const result = await dedupeProfiles(null, null);
      setOperationMessage(
        t("panes.profiles.dedupe.removed", {
          kept: result.kept,
          removed: result.removedProfileIds.length,
          total: result.total,
        }),
      );
    } catch (error) {
      setOperationError(getErrorMessage(error));
    }
  }

  function confirmDelete() {
    const indexIds = pendingDelete;
    setPendingDelete(null);
    if (indexIds && indexIds.length > 0) {
      if (selectedId && indexIds.includes(selectedId)) {
        setSelectedId(null);
      }
      void runOperation(() => deleteProfiles(indexIds));
    }
  }

  function selectOnly(indexId: string) {
    setSelectedId(indexId);
  }

  async function handleSort(sortKey: ProfileSortKey) {
    const ascending = sortState?.key === sortKey ? !sortState.ascending : true;
    setSortState({ ascending, key: sortKey });
    await runOperation(() => sortProfiles(null, sortKey, ascending));
  }

  async function handleSave(profile: Profile) {
    const save = profile.protocol.kind === CONFIG_TYPES.PolicyGroup || profile.protocol.kind === CONFIG_TYPES.ProxyChain
      ? saveGroupProfile
      : saveProfile;
    setSaveError(null);
    // The editor remounts its form whenever `open` toggles, so closing it on a
    // rejected save would discard every in-progress edit.
    if (await runOperation(() => save(profile), setSaveError)) {
      setDialogStateInternal(null);
    }
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

      const result = await importProfilesFromText(text, null);
      await handleDialogImport(result);
    } catch (error) {
      setOperationError(getErrorMessage(error));
    } finally {
      setImportingFromClipboard(false);
    }
  }

  async function handleDialogImport(result: ImportProfilesResult) {
    setOperationError(null);
    setOperationMessage(formatImportSummary(result, t));
    const importedIndexIds = result.importedProfileIds;
    if (importedIndexIds.length > 0) {
      setFilterText("");
      setSelectedId(importedIndexIds[0] ?? null);
      // Optimistic: jump straight to the unfiltered list the caller was just
      // switched to. `import_profiles_from_text` still emits profiles +
      // subscriptions + subscriptionMetadata for every other cache.
      const refreshedProfiles = await listProfiles(null, null);
      queryClient.setQueryData(profilesQueryKey(""), refreshedProfiles);
    }
  }

  async function performExport(
    kind: ProfileExportKind,
    indexIds: string[],
    showQr: boolean,
    saveFile: boolean,
  ) {
    const result = await runProfileExport(kind, indexIds);
    if (showQr) {
      setShareQrContent(result.text);
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
  }

  async function handleExport(kind: ProfileExportKind, indexIds: string[], showQr = false, saveFile = false) {
    setOperationError(null);
    setOperationMessage(null);
    if (indexIds.length === 0) {
      setOperationError(t("panes.profiles.export.noSelection"));
      return;
    }

    try {
      await performExport(kind, indexIds, showQr, saveFile);
    } catch (error) {
      setOperationError(getErrorMessage(error));
    }
  }

  async function handleBulkExport(kind: ProfileExportKind, showQr = false, saveFile = false) {
    setOperationError(null);
    setOperationMessage(null);
    try {
      const allProfiles = (await listProfiles(null, null)).entries;
      // Share-link exporters reject group/chain/custom/HTTP profiles, and the
      // backend fails the whole batch on the first rejection, so those profiles
      // are dropped here instead of breaking the export for everyone else.
      const exportable = isShareLinkExport(kind)
        ? allProfiles.filter((item) => supportsShareLinkExport(item.profile.protocol.kind))
        : allProfiles;
      const skipped = allProfiles.length - exportable.length;
      const indexIds = exportable.map((item) => item.profile.id);
      if (indexIds.length === 0) {
        setOperationError(t("panes.profiles.export.noProfiles"));
        return;
      }
      await performExport(kind, indexIds, showQr, saveFile);
      if (skipped > 0) {
        setOperationMessage(t("panes.profiles.export.skippedUnsupported", { count: skipped }));
      }
    } catch (error) {
      setOperationError(getErrorMessage(error));
    }
  }

  async function handleSpeedtest(kind: SpeedtestKind, target: SpeedtestTarget) {
    setColumnVisibility((current) => ({ ...current, delay: true, speed: true }));
    setSpeedtestRunning(true);
    try {
      await runOperation(() => runSpeedtest({
        kind,
        target,
      }));
    } finally {
      setSpeedtestRunning(false);
    }
  }

  async function handleCancelSpeedtest() {
    await runOperation(() => cancelSpeedtest());
    setSpeedtestRunning(false);
  }

  return {
    confirmDedupe,
    confirmDelete,
    dialogState,
    filterText,
    gridMinWidth,
    gridTemplateColumns,
    handleBulkExport,
    handleCancelSpeedtest,
    handleDialogImport,
    handleExport,
    handleImportFromClipboard,
    handleSave,
    handleSort,
    handleSpeedtest,
    hideableColumns,
    importOpen,
    importingFromClipboard,
    operationError,
    operationMessage,
    pendingDedupe,
    pendingDelete,
    profiles,
    profilesQuery,
    renderedRows,
    requestDedupe,
    requestDelete,
    resetColumnVisibility,
    rows,
    rowVirtualizer,
    runOperation,
    saveError,
    selectOnly,
    selectedId,
    setDialogState,
    setFilterText,
    setImportOpen,
    setPendingDedupe,
    setPendingDelete,
    setShareQrContent,
    setSubscriptionsOpen,
    shareQrContent,
    sortState,
    speedtestRunning,
    subscriptionsOpen,
    t,
    undecodableProfiles,
    viewportRef,
    visibleColumns,
  };
}

export type ServerTableController = ReturnType<typeof useServerTable>;
