import { useI18n } from "@voya/i18n/use-i18n";
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
  const imports = useNodeImport(operation, editor.handleDialogImport, t);
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
  };
}
