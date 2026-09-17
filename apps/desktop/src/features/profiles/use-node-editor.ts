import { useRef, useState, type RefObject } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { restoreFocus } from "@voya/ui/lib/focus";
import { deleteProfiles, listProfiles, saveProfile } from "@/ipc/commands";
import type { ImportProfilesResult, Profile, ProfileListEntry } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { useProfileActivation } from "@/stores/runtime-action";
import { formatImportSummary } from "./server-table-actions";
import type { DialogImportMethod } from "./import-methods";
import type { TranslationFunction } from "@voya/i18n";
import type { NodeOperation } from "./use-node-operation";
type DialogState =
  | { mode: "create"; profile?: null }
  | { mode: "edit"; profile: ProfileListEntry }
  | null;

export function useNodeEditor(
  { runOperation, setOperationError, setOperationMessage }: NodeOperation,
  viewportRef: RefObject<HTMLDivElement | null>,
  t: TranslationFunction,
) {
  const [dialogState, setDialogStateInternal] = useState<DialogState>(null);
  const [importMethod, setImportMethod] = useState<DialogImportMethod | null>(null);
  const profileDialogTriggerRef = useRef<HTMLElement | null>(null);
  const addTriggerRef = useRef<HTMLButtonElement>(null);
  const [pendingDelete, setPendingDelete] = useState<string[] | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [detailsId, setDetailsId] = useState<string | null>(null);
  const detailsTriggerRef = useRef<HTMLButtonElement | null>(null);
  const activation = useProfileActivation(t);
  const queryClient = useQueryClient();
  function openDetails(id: string, trigger: HTMLButtonElement) {
    detailsTriggerRef.current = trigger;
    setDetailsId(id);
  }
  function restoreDetailsFocus() {
    restoreFocus(detailsTriggerRef.current, viewportRef.current);
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

  async function handleDialogImport(result: ImportProfilesResult, isActive = () => true) {
    if (!isActive()) return;
    setOperationError(null);
    setOperationMessage(formatImportSummary(result, t));
    const importedIndexIds = result.importedProfileIds;
    if (importedIndexIds.length > 0) {
      // Refresh the complete list after import. `import_profiles_from_text` still emits profiles +
      // subscriptions + subscriptionMetadata for every other cache.
      const refreshedProfiles = await listProfiles(null, null);
      if (isActive()) queryClient.setQueryData(queryKeys.profileList, refreshedProfiles);
    }
  }

  function restoreProfileDialogFocus() {
    restoreFocus(profileDialogTriggerRef.current, viewportRef.current);
  }

  return {
    dialogState,
    setDialogState,
    importMethod,
    setImportMethod,
    addTriggerRef,
    pendingDelete,
    setPendingDelete,
    saveError,
    activation,
    detailsId,
    setDetailsId,
    openDetails,
    restoreDetailsFocus,
    requestDelete,
    confirmDelete,
    handleSave,
    handleDialogImport,
    restoreProfileDialogFocus,
  };
}
