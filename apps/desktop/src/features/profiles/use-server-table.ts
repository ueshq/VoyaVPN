import { useRef } from "react";

import { useI18n } from "@voya/i18n/use-i18n";
import type { ImportProfilesResult } from "@voya/contracts";
import { useProfileActivation } from "@voya/client/runtime-action";
import { useNodeSelection } from "@voya/features/profiles/use-node-selection";
import { useNodeSpeedtest } from "@voya/features/profiles/use-node-speedtest";
import { useNodeOperation } from "@voya/features/profiles/use-node-operation";
import { useNodeListData } from "@voya/features/profiles/use-node-list-data";
import { useNodeListVirtual } from "./use-node-list-virtual";
import { useNodeEditor } from "./use-node-editor";
import { useNodeSubscriptions } from "@/features/profiles/use-node-subscriptions";
import { useNodeExport } from "@voya/features/profiles/use-node-export";
import { useNodeImport } from "@/features/profiles/use-node-import";
import { usePolicyGroups } from "@voya/features/profiles/use-policy-groups";

/** Compose page capabilities; individual components consume only their own facet. */
export function useServerTable() {
  const { t } = useI18n();
  // The search input's ref is what the list's clear button refocuses.
  const searchRef = useRef<HTMLInputElement>(null);
  const nodeGroups = { ...useNodeSelection(searchRef), searchRef };
  const operation = useNodeOperation();
  const data = useNodeListData(nodeGroups, t);
  const listView = useNodeListVirtual(data.rows, data.search);
  const editor = useNodeEditor(operation, listView.viewportRef, t);
  const activation = useProfileActivation(t);
  // The desktop restores focus to the control that opened a dialog.
  const subscriptions = useNodeSubscriptions<HTMLElement>(operation, t);
  const exports = useNodeExport(operation, t);
  // A subscription URL only creates the source; updating it straight away is
  // what brings its nodes in, so the user never meets an empty group.
  async function handleImported(
    result: ImportProfilesResult,
    isActive: () => boolean = () => true,
  ) {
    await editor.handleDialogImport(result, isActive);
    await subscriptions.updateImportedSubscriptions(result.addedSubscriptionIds, isActive);
  }
  const imports = useNodeImport(operation, handleImported, t);
  const speedtest = useNodeSpeedtest(operation);
  const policyGroups = usePolicyGroups(operation, t);
  return {
    t,
    nodeGroups,
    activation,
    ...operation,
    ...data,
    ...listView,
    ...editor,
    ...subscriptions,
    ...exports,
    ...speedtest,
    ...imports,
    ...policyGroups,
    handleImported,
  };
}

export type ServerTableController = ReturnType<typeof useServerTable>;
