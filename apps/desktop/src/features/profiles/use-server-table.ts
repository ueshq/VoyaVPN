import { useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";

import {
  cancelSpeedtest,
  deleteProfiles,
  listProfiles,
  listSubscriptions,
  runSpeedtest,
  saveProfile,
  saveTextFile,
  useRuntimeEventStore,
} from "@/ipc";
import type {
  ImportProfilesResult,
  Profile,
  ProfileListEntry,
  SpeedtestTarget,
} from "@/ipc/bindings";
import { profilesQueryKey, queryKeys } from "@/ipc/query-keys";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { useProfileActivation } from "@/features/home/use-profile-activation";
import { useNodeGroups } from "./use-node-groups";
import { nodeListRows } from "./node-list-rows";

import {
  exportFileFilter,
  exportFileName,
  formatImportSummary,
  isShareLinkExport,
  runProfileExport,
  supportsShareLinkExport,
  type ProfileExportKind,
} from "./server-table-actions";
import type { ImportMethod } from "./import-methods";
import { applyLiveUpdates } from "./server-table-live-updates";

type DialogState =
  | { mode: "create"; profile?: null }
  | { mode: "edit"; profile: ProfileListEntry }
  | null;

export function useServerTable() {
  const nodeGroups = useNodeGroups();
  const [dialogState, setDialogStateInternal] = useState<DialogState>(null);
  const [filterText, setFilterText] = useState("");
  const [importMethod, setImportMethod] = useState<ImportMethod | null>(null);
  const profileDialogTriggerRef = useRef<HTMLElement | null>(null);
  const addTriggerRef = useRef<HTMLButtonElement>(null);
  const importTriggerRef = useRef<HTMLButtonElement>(null);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [operationMessage, setOperationMessage] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<string[] | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [shareQrContent, setShareQrContent] = useState<string | null>(null);
  const [subscriptionsOpen, setSubscriptionsOpen] = useState(false);
  const { t } = useI18n();
  const [detailsId, setDetailsId] = useState<string | null>(null);
  const detailsTriggerRef = useRef<HTMLButtonElement | null>(null);
  const activation = useProfileActivation(t, { onSelect: setSelectedId });
  const subscriptionsQuery = useQuery({
    queryFn: listSubscriptions,
    queryKey: queryKeys.subscriptions,
  });
  const subscriptionNames = useMemo(() => new Map(
    (subscriptionsQuery.data ?? []).map((item) => [item.id, item.remarks || t("panes.subscriptions.untitled")]),
  ), [subscriptionsQuery.data, t]);
  const speedtestResultsByProfileId = useRuntimeEventStore((state) => state.speedtestResultsByProfileId);
  const speedtestRunning = useRuntimeEventStore((state) => state.speedtestRunning);
  const setSpeedtestRunning = useRuntimeEventStore((state) => state.setSpeedtestRunning);
  const queryClient = useQueryClient();
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: profilesQueryKey(""),
  });
  const profiles = useMemo(
    () =>
      applyLiveUpdates(
        profilesQuery.data?.entries ?? [],
        undefined,
        speedtestResultsByProfileId,
      ),
    [profilesQuery.data, speedtestResultsByProfileId],
  );
  // Stored servers this build could not read. The backend skips those rows so
  // one of them cannot hide every other server, and reports how many it
  // skipped; the toolbar states it, because a list that is quietly short is
  // indistinguishable from data loss.
  const undecodableProfiles = profilesQuery.data?.undecodableProfiles ?? 0;

  const viewportRef = useRef<HTMLDivElement>(null);
  const rows = useMemo(() => nodeListRows(profiles, nodeGroups.snapshot, nodeGroups.expanded, filterText, t("nodeGroups.unassigned")), [profiles, nodeGroups.snapshot, nodeGroups.expanded, filterText, t]);
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    estimateSize: () => 100,
    getItemKey: (index) => rows[index]!.key,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 520, width: 1200 },
    overscan: 5,
  });
  const visibleRows = rowVirtualizer.getVirtualItems();
  const renderedRows = visibleRows.length > 0 ? visibleRows : rows.slice(0, 15).map((row, index) => ({
    index, key: row.key, start: index * 100,
  }));
  function subscriptionName(item: ProfileListEntry) {
    return item.profile.subscriptionId
      ? subscriptionNames.get(item.profile.subscriptionId) ?? t("panes.subscriptions.untitled")
      : t("panes.profiles.card.local");
  }
  function openDetails(id: string, trigger: HTMLButtonElement) {
    detailsTriggerRef.current = trigger;
    setDetailsId(id);
  }
  function restoreDetailsFocus() {
    const trigger = detailsTriggerRef.current;
    if (trigger?.isConnected) trigger.focus();
    else viewportRef.current?.focus();
  }
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
    if (next) {
      profileDialogTriggerRef.current = next.mode === "create" ? addTriggerRef.current
        : document.activeElement instanceof HTMLElement ? document.activeElement : null;
    }
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

  async function handleSave(profile: Profile) {
    setSaveError(null);
    // The editor remounts its form whenever `open` toggles, so closing it on a
    // rejected save would discard every in-progress edit.
    if (await runOperation(() => saveProfile(profile), setSaveError)) {
      setDialogStateInternal(null);
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
        filters: [exportFileFilter(kind, t)],
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

  async function handleSpeedtest(target: SpeedtestTarget) {
    if (useRuntimeEventStore.getState().speedtestRunning) return;
    setSpeedtestRunning(true);
    try {
      await runOperation(() => runSpeedtest({
        target,
      }));
    } finally {
      setSpeedtestRunning(false);
    }
  }

  async function handleCancelSpeedtest() {
    await runOperation(async () => {
      const status = await cancelSpeedtest();
      useRuntimeEventStore.getState().setSpeedtestStatus(status);
    });
  }

  return {
    nodeGroups,
    rows,
    activation,
    detailsId,
    openDetails,
    restoreDetailsFocus,
    setDetailsId,
    subscriptionName,
    confirmDelete,
    dialogState,
    filterText,
    handleBulkExport,
    handleCancelSpeedtest,
    handleDialogImport,
    handleExport,
    handleSave,
    handleSpeedtest,
    importMethod,
    addTriggerRef,
    restoreProfileDialogFocus: () => {
      const trigger = profileDialogTriggerRef.current;
      if (trigger?.isConnected) trigger.focus();
      else viewportRef.current?.focus();
    },
    importTriggerRef,
    operationError,
    operationMessage,
    pendingDelete,
    profiles,
    profilesQuery,
    renderedRows,
    requestDelete,
    rowVirtualizer,
    runOperation,
    saveError,
    selectOnly,
    selectedId,
    setDialogState,
    setFilterText,
    setImportMethod,
    setPendingDelete,
    setShareQrContent,
    setSubscriptionsOpen,
    shareQrContent,
    speedtestRunning,
    subscriptionsOpen,
    t,
    undecodableProfiles,
    viewportRef,
  };
}

export type ServerTableController = ReturnType<typeof useServerTable>;
