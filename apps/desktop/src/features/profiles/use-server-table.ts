import { useI18n } from "@voya/i18n/use-i18n";
import type { ImportProfilesResult } from "@/ipc/bindings";
import { useNodeGroups } from "./use-node-groups";
import { useNodeOperation } from "@voya/features/profiles/use-node-operation";
import { useNodeListData } from "@voya/features/profiles/use-node-list-data";
import { useNodeListVirtual } from "./use-node-list-virtual";
import { useNodeEditor } from "./use-node-editor";
import { useNodeSubscriptions } from "@voya/features/profiles/use-node-subscriptions";
import { useNodeExport } from "@voya/features/profiles/use-node-export";
import { useNodeImport } from "@voya/features/profiles/use-node-import";
import { useNodeSpeedtest } from "./use-node-speedtest";
import { usePolicyGroups } from "@voya/features/profiles/use-policy-groups";

/** Compose page capabilities; individual components consume only their own facet. */
export function useServerTable() {
  const { t } = useI18n();
  const nodeGroups = useNodeGroups();
  const operation = useNodeOperation();
  const data = useNodeListData(nodeGroups, t);
  const listView = useNodeListVirtual(data.rows, data.search);
  const editor = useNodeEditor(operation, listView.viewportRef, t);
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
    for (const id of result.addedSubscriptionIds) {
      if (!isActive()) return;
      await subscriptions.updateSubscription(id);
    }
  }
  const imports = useNodeImport(operation, handleImported, t);
  const speedtest = useNodeSpeedtest(operation);
  const policyGroups = usePolicyGroups(operation, t);
  return {
    t,
    nodeGroups,
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
