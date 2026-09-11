import { metadataBySubscriptionId } from "@/features/subscriptions/subscription-usage";
import { useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";

import {
  cancelSpeedtest,
  deleteProfiles,
  listProfiles,
  listNodeGroups,
  listSubscriptions,
  listSubscriptionMetadata,
  updateSubscriptions,
  deleteSubscriptions,
  runSpeedtest,
  saveProfile,
} from "@/ipc/commands";
import { saveTextFile } from "@/ipc/file-dialog";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type {
  ImportProfilesResult,
  Profile,
  ProfileListEntry,
  SpeedtestResult,
  SpeedtestTarget,
  Subscription,
} from "@/ipc/bindings";
import {
  isSubscriptionUpdateFailure,
  subscriptionUpdateMessages,
} from "@/features/subscriptions/subscription-update-result";
import { profilesQueryKey, queryKeys } from "@/ipc/query-keys";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { useProfileActivation } from "@/features/home/use-profile-activation";
import { useNodeGroups } from "./use-node-groups";
import { nodeListRows, profilesByNodeGroup } from "./node-list-rows";

import {
  exportFileFilter,
  exportFileName,
  formatImportSummary,
  isShareLinkExport,
  runProfileExport,
  supportsShareLinkExport,
  type ProfileExportKind,
  type ProfileExportDestination,
} from "./server-table-actions";
import type { ImportMethod } from "./import-methods";

type DialogState =
  | { mode: "create"; profile?: null }
  | { mode: "edit"; profile: ProfileListEntry }
  | null;

export function useServerTable() {
  const nodeGroups = useNodeGroups();
  const [dialogState, setDialogStateInternal] = useState<DialogState>(null);
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
  const [editingSubscription, setEditingSubscription] =
    useState<Subscription | null>(null);
  const [deletingSubscription, setDeletingSubscription] =
    useState<Subscription | null>(null);
  const [deletingSubscriptionPending, setDeletingSubscriptionPending] =
    useState(false);
  const deletingSubscriptionRef = useRef(false);
  const [updatingSubscriptions, setUpdatingSubscriptions] = useState<
    Set<string>
  >(() => new Set());
  const updatingRef = useRef(new Set<string>());
  const subscriptionTriggerRef = useRef<HTMLElement | null>(null);
  const metadataQuery = useQuery({
    queryFn: listSubscriptionMetadata,
    queryKey: queryKeys.subscriptionMetadata,
  });
  const subscriptionMetadata = useMemo(
    () => metadataBySubscriptionId(metadataQuery.data ?? []),
    [metadataQuery.data],
  );
  function openSubscription(
    subscription: Subscription | null,
    trigger?: HTMLElement,
  ) {
    subscriptionTriggerRef.current =
      trigger ??
      (document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null);
    setEditingSubscription(subscription);
    setSubscriptionsOpen(true);
  }
  async function updateSubscription(id: string) {
    if (updatingRef.current.has(id)) return;
    updatingRef.current.add(id);
    setUpdatingSubscriptions(new Set(updatingRef.current));
    try {
      await runOperation(async () => {
        const result = await updateSubscriptions(id, true, null);
        if (isSubscriptionUpdateFailure(result))
          throw new Error(
            subscriptionUpdateMessages(result) ||
              t("panes.subscriptions.updateNothingImported"),
          );
        setOperationMessage(
          t("panes.subscriptions.updateResult", {
            imported: result.imported,
            updated: result.updated,
          }),
        );
      });
    } finally {
      updatingRef.current.delete(id);
      setUpdatingSubscriptions(new Set(updatingRef.current));
    }
  }
  async function removeSubscription() {
    if (!deletingSubscription || deletingSubscriptionRef.current) return;
    deletingSubscriptionRef.current = true;
    setDeletingSubscriptionPending(true);
    try {
      if (
        await runOperation(() => deleteSubscriptions([deletingSubscription.id]))
      ) {
        setDeletingSubscription(null);
      }
    } finally {
      deletingSubscriptionRef.current = false;
      setDeletingSubscriptionPending(false);
    }
  }
  function confirmSubscriptionDeletion(
    subscription: Subscription,
    trigger: HTMLElement,
  ) {
    subscriptionTriggerRef.current = trigger;
    setOperationError(null);
    setDeletingSubscription(subscription);
  }
  const { t } = useI18n();
  const [detailsId, setDetailsId] = useState<string | null>(null);
  const detailsTriggerRef = useRef<HTMLButtonElement | null>(null);
  const activation = useProfileActivation(t, { onSelect: setSelectedId });
  const subscriptionsQuery = useQuery({
    queryFn: listSubscriptions,
    queryKey: queryKeys.subscriptions,
  });
  const subscriptionNames = useMemo(
    () =>
      new Map(
        (subscriptionsQuery.data ?? []).map((item) => [
          item.id,
          item.remarks || t("panes.subscriptions.untitled"),
        ]),
      ),
    [subscriptionsQuery.data, t],
  );
  const speedtestResultsByProfileId = useRuntimeEventStore(
    (state) => state.speedtestResultsByProfileId,
  );
  const speedtestRunning = useRuntimeEventStore(
    (state) => state.speedtestRunning,
  );
  const setSpeedtestRunning = useRuntimeEventStore(
    (state) => state.setSpeedtestRunning,
  );
  const queryClient = useQueryClient();
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: profilesQueryKey(""),
  });
  const profiles = useMemo(
    () =>
      applySpeedtestResults(
        profilesQuery.data?.entries ?? [],
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
  const rows = useMemo(
    () =>
      nodeListRows(
        profiles,
        nodeGroups.snapshot,
        nodeGroups.collapsed,
        t("nodeGroups.unassigned"),
        subscriptionsQuery.data,
        t("panes.subscriptions.untitled"),
      ),
    [
      profiles,
      nodeGroups.snapshot,
      nodeGroups.collapsed,
      subscriptionsQuery.data,
      t,
    ],
  );
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    estimateSize: () => 100,
    getItemKey: (index) => rows[index]!.key,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 520, width: 1200 },
    overscan: 5,
  });
  const visibleRows = rowVirtualizer.getVirtualItems();
  const renderedRows =
    visibleRows.length > 0
      ? visibleRows
      : rows.slice(0, 15).map((row, index) => ({
          index,
          key: row.key,
          start: index * 100,
        }));
  function subscriptionName(item: ProfileListEntry) {
    return item.profile.subscriptionId
      ? (subscriptionNames.get(item.profile.subscriptionId) ??
          t("panes.subscriptions.untitled"))
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
      // Mutating commands emit their own cache invalidations.
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
      profileDialogTriggerRef.current =
        next.mode === "create"
          ? addTriggerRef.current
          : document.activeElement instanceof HTMLElement
            ? document.activeElement
            : null;
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
      setSelectedId(importedIndexIds[0] ?? null);
      // Refresh the complete list after import. `import_profiles_from_text` still emits profiles +
      // subscriptions + subscriptionMetadata for every other cache.
      const refreshedProfiles = await listProfiles(null, null);
      queryClient.setQueryData(profilesQueryKey(""), refreshedProfiles);
    }
  }

  async function performExport(
    kind: ProfileExportKind,
    indexIds: string[],
    destination: ProfileExportDestination,
  ) {
    const result = await runProfileExport(kind, indexIds);
    if (destination === "qr") {
      setShareQrContent(result.text);
      return true;
    }

    if (destination === "file") {
      const path = await saveTextFile({
        defaultPath: exportFileName(kind),
        filters: [exportFileFilter(kind, t)],
        text: result.text,
      });
      if (path) {
        setOperationMessage(t("panes.profiles.export.savedFile", { path }));
      }
      return !!path;
    }

    if (!navigator.clipboard?.writeText) {
      throw new Error(t("panes.profiles.export.clipboardUnavailable"));
    }
    await navigator.clipboard.writeText(result.text);
    setOperationMessage(
      t("panes.profiles.export.copied", { count: result.count }),
    );
    return true;
  }

  async function handleExport(
    kind: ProfileExportKind,
    indexIds: string[],
    destination: ProfileExportDestination = "clipboard",
  ) {
    await runOperation(async () => {
      if (indexIds.length === 0) {
        setOperationError(t("panes.profiles.export.noSelection"));
        return;
      }
      await performExport(kind, indexIds, destination);
    });
  }

  async function handleBulkExport(
    kind: ProfileExportKind,
    destination: ProfileExportDestination = "clipboard",
  ) {
    await runOperation(async () => {
      const allProfiles = (await listProfiles(null, null)).entries;
      await performBatchExport(kind, allProfiles, destination);
    });
  }

  async function performBatchExport(
    kind: ProfileExportKind,
    entries: ProfileListEntry[],
    destination: ProfileExportDestination,
  ) {
    const exportable = isShareLinkExport(kind)
      ? entries.filter((item) =>
          supportsShareLinkExport(item.profile.protocol.kind),
        )
      : entries;
    if (!exportable.length) {
      setOperationError(t("panes.profiles.export.noProfiles"));
      return;
    }
    const completed = await performExport(
      kind,
      exportable.map((item) => item.profile.id),
      destination,
    );
    const skipped = entries.length - exportable.length;
    if (completed && skipped > 0)
      setOperationMessage(
        t("panes.profiles.export.skippedUnsupported", { count: skipped }),
      );
  }

  async function handleGroupExport(
    groupKey: string,
    kind: ProfileExportKind,
    destination: ProfileExportDestination = "clipboard",
  ) {
    await runOperation(async () => {
      const [listing, snapshot] = await Promise.all([
        listProfiles(null, null),
        listNodeGroups(),
      ]);
      if (groupKey.startsWith("subscription:")) {
        await performBatchExport(
          kind,
          listing.entries.filter(
            (entry) => entry.profile.subscriptionId === groupKey.slice(13),
          ),
          destination,
        );
        return;
      }
      const groupId = groupKey.startsWith("manual:") ? groupKey.slice(7) : null;
      if (
        groupId !== null &&
        !snapshot.groups.some((group) => group.id === groupId)
      ) {
        setOperationError(t("nodeGroups.notFound"));
        return;
      }
      await performBatchExport(
        kind,
        profilesByNodeGroup(listing.entries, snapshot).get(groupId) ?? [],
        destination,
      );
    });
  }

  async function handleSpeedtest(target: SpeedtestTarget) {
    if (useRuntimeEventStore.getState().speedtestRunning) return;
    setSpeedtestRunning(true);
    try {
      await runOperation(() =>
        runSpeedtest({
          target,
        }),
      );
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
    editingSubscription,
    deletingSubscription,
    deletingSubscriptionPending,
    setDeletingSubscription,
    removeSubscription,
    confirmSubscriptionDeletion,
    openSubscription,
    updateSubscription,
    updatingSubscriptions,
    subscriptionTriggerRef,
    subscriptionsQuery,
    subscriptionMetadata,
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
    handleBulkExport,
    handleGroupExport,
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
    setSelectedId,
    selectedId,
    setDialogState,
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

export function applySpeedtestResults(
  profiles: ProfileListEntry[],
  speedtestResults: Record<string, SpeedtestResult>,
) {
  if (Object.keys(speedtestResults).length === 0) return profiles;

  let changed = false;
  const nextProfiles = profiles.map((item) => {
    const result = speedtestResults[item.profile.id];
    if (!result) return item;

    changed = true;
    return {
      ...item,
      metrics: {
        ...item.metrics,
        // Country stays on the query snapshot; cached events may outlive edits.
        delayMs: result.delay ?? item.metrics.delayMs,
        ipInfo: result.ipInfo ?? item.metrics.ipInfo,
        outcome: result.outcome,
      },
    };
  });

  return changed ? nextProfiles : profiles;
}
