import { useRef, useState, type RefObject } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { restoreFocus } from "@voya/ui/lib/focus";
import { voyaCommands } from "@voya/client/transport";
import type { ImportProfilesResult, Profile } from "@voya/contracts";
import { queryKeys } from "@voya/client/query-keys";
import { formatImportSummary } from "@voya/features/profiles/server-table-actions";
import type { DialogImportMethod } from "@/features/profiles/import-methods";
import type { TranslationFunction } from "@voya/i18n";
import type { NodeOperation } from "@voya/features/profiles/use-node-operation";
type DialogState =
  | { mode: "create"; profile?: null }
  | { mode: "edit"; profile: Profile }
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
  const openEditorRequestRef = useRef(0);
  const [pendingDelete, setPendingDelete] = useState<string[] | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [detailsId, setDetailsId] = useState<string | null>(null);
  const detailsTriggerRef = useRef<HTMLButtonElement | null>(null);
  const queryClient = useQueryClient();
  function openDetails(id: string, trigger: HTMLButtonElement) {
    detailsTriggerRef.current = trigger;
    setDetailsId(id);
  }
  function restoreDetailsFocus() {
    restoreFocus(detailsTriggerRef.current, viewportRef.current);
  }
  // Opening or closing the editor drops the previous rejection message.
  function setDialogState(next: DialogState, trigger: HTMLElement | null = focusedElement()) {
    if (next) {
      profileDialogTriggerRef.current = next.mode === "create" ? addTriggerRef.current : trigger;
    }
    setSaveError(null);
    setDialogStateInternal(next);
  }

  // The list carries summaries, so the node is read in full first. The menu
  // item that asked has focus now and may not by the time the read returns,
  // so it is kept for the editor to hand focus back to.
  //
  // Two reads can overlap (a row menu, then another node's before the first
  // answers) and they can answer in either order, so only the newest one is
  // allowed to open the editor: otherwise the user asks for B and gets A.
  async function openEditor(indexId: string) {
    const request = ++openEditorRequestRef.current;
    const trigger = focusedElement();
    const details = voyaCommands().getProfile(indexId);
    if (await runOperation(() => details)) {
      if (request !== openEditorRequestRef.current) return;
      setDialogState({ mode: "edit", profile: (await details).profile }, trigger);
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
      void runOperation(() => voyaCommands().deleteProfiles(indexIds));
    }
  }

  async function handleSave(profile: Profile) {
    setSaveError(null);
    // The editor remounts its form whenever `open` toggles, so closing it on a
    // rejected save would discard every in-progress edit.
    if (await runOperation(() => voyaCommands().saveProfile(profile), setSaveError)) {
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
      const refreshedProfiles = await voyaCommands().listProfileSummaries();
      if (isActive()) queryClient.setQueryData(queryKeys.profileList, refreshedProfiles);
    }
  }

  function restoreProfileDialogFocus() {
    restoreFocus(profileDialogTriggerRef.current, viewportRef.current);
  }

  return {
    dialogState,
    setDialogState,
    openEditor,
    importMethod,
    setImportMethod,
    addTriggerRef,
    pendingDelete,
    setPendingDelete,
    saveError,
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

function focusedElement() {
  return document.activeElement instanceof HTMLElement ? document.activeElement : null;
}
