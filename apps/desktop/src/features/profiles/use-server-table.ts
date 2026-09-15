import { useI18n } from "@voya/i18n/use-i18n";
import type { ImportProfilesResult } from "@/ipc/bindings";
import { useNodeGroups } from "./use-node-groups";
import { useNodeOperation } from "./use-node-operation";
import { useNodeListData } from "./use-node-list-data";
import { useNodeEditor } from "./use-node-editor";
import { useNodeSubscriptions } from "./use-node-subscriptions";
import { useNodeExport } from "./use-node-export";
import { useNodeImport } from "./use-node-import";
import { useNodeSpeedtest } from "./use-node-speedtest";
import { usePolicyGroups } from "./use-policy-groups";

/** Compose page capabilities; individual components consume only their own facet. */
export function useServerTable() {
  const { t } = useI18n();
  const nodeGroups = useNodeGroups();
  const operation = useNodeOperation();
  const data = useNodeListData(nodeGroups, t);
  const editor = useNodeEditor(operation, data.viewportRef, t);
  const subscriptions = useNodeSubscriptions(operation, t);
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
