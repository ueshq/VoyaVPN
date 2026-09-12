import type { TranslationFunction } from "@voya/i18n";
import type { useNodeGroups } from "./use-node-groups";
import type { NodeOperation } from "./use-node-operation";
import type { useNodeListData } from "./use-node-list-data";
import type { useNodeEditor } from "./use-node-editor";
import type { useNodeSubscriptions } from "./use-node-subscriptions";
import type { useNodeExport } from "./use-node-export";
import type { useNodeImport } from "./use-node-import";
import type { useNodeSpeedtest } from "./use-node-speedtest";

type NodeListData = ReturnType<typeof useNodeListData>;
type NodeEditor = ReturnType<typeof useNodeEditor>;
type NodeSubscriptions = ReturnType<typeof useNodeSubscriptions>;
type NodeExport = ReturnType<typeof useNodeExport>;
type NodeSpeedtest = ReturnType<typeof useNodeSpeedtest>;
type NodeShared = { t: TranslationFunction; nodeGroups: ReturnType<typeof useNodeGroups> };

export type NodeMenuController = Pick<NodeOperation, "runOperation"> &
  Pick<NodeEditor, "requestDelete" | "setDialogState" | "setSelectedId"> &
  Pick<NodeExport, "handleExport"> &
  Pick<NodeSpeedtest, "handleSpeedtest" | "speedtestRunning"> &
  Pick<NodeShared, "t">;

export type NodeDetailsController = Pick<NodeListData, "subscriptionName"> &
  Pick<NodeEditor, "restoreDetailsFocus" | "setDetailsId"> &
  Pick<NodeShared, "t">;

export type NodeGroupCardController = Pick<NodeListData, "subscriptionMetadata"> &
  Pick<
    NodeSubscriptions,
    | "confirmSubscriptionDeletion"
    | "openSubscription"
    | "updateSubscription"
    | "updatingSubscriptions"
  > &
  Pick<NodeExport, "handleGroupExport"> &
  Pick<NodeSpeedtest, "handleCancelSpeedtest" | "handleSpeedtest" | "speedtestRunning"> &
  Pick<NodeShared, "nodeGroups" | "t">;

export type NodeDialogsController = Pick<NodeOperation, "operationError"> &
  Pick<NodeListData, "profiles" | "subscriptionName" | "viewportRef"> &
  Pick<
    NodeEditor,
    | "confirmDelete"
    | "detailsId"
    | "dialogState"
    | "handleDialogImport"
    | "handleSave"
    | "importMethod"
    | "importTriggerRef"
    | "pendingDelete"
    | "restoreDetailsFocus"
    | "restoreProfileDialogFocus"
    | "saveError"
    | "setDetailsId"
    | "setDialogState"
    | "setImportMethod"
    | "setPendingDelete"
  > &
  Pick<
    NodeSubscriptions,
    | "deletingSubscription"
    | "deletingSubscriptionPending"
    | "editingSubscription"
    | "removeSubscription"
    | "setDeletingSubscription"
    | "setSubscriptionsOpen"
    | "subscriptionTriggerRef"
    | "subscriptionsOpen"
  > &
  Pick<NodeExport, "setShareQrContent" | "shareQrContent"> &
  Pick<NodeShared, "t">;

export type NodeNoticesController = Pick<ReturnType<typeof useNodeImport>, "directImportPending"> &
  Pick<NodeOperation, "operationError" | "operationMessage"> &
  Pick<NodeListData, "profilesQuery" | "undecodableProfiles"> &
  Pick<NodeShared, "t">;

export type NodeToolbarController = Pick<ReturnType<typeof useNodeImport>, "handleDirectImport" | "directImportPending"> &
  Pick<NodeListData, "profiles" | "profilesQuery"> &
  Pick<NodeEditor, "addTriggerRef" | "importTriggerRef" | "setDialogState" | "setImportMethod"> &
  Pick<NodeSubscriptions, "openSubscription"> &
  Pick<NodeExport, "handleBulkExport"> &
  Pick<NodeSpeedtest, "handleCancelSpeedtest" | "handleSpeedtest" | "speedtestRunning"> &
  Pick<NodeShared, "t">;

export type NodeListController = Pick<NodeOperation, "runOperation"> &
  Pick<
    NodeListData,
    | "profilesQuery"
    | "renderedRows"
    | "rowVirtualizer"
    | "rows"
    | "subscriptionMetadata"
    | "viewportRef"
  > &
  Pick<
    NodeEditor,
    | "activation"
    | "openDetails"
    | "requestDelete"
    | "selectedId"
    | "setDialogState"
    | "setSelectedId"
  > &
  Pick<
    NodeSubscriptions,
    | "confirmSubscriptionDeletion"
    | "openSubscription"
    | "updateSubscription"
    | "updatingSubscriptions"
  > &
  Pick<NodeExport, "handleExport" | "handleGroupExport"> &
  Pick<NodeSpeedtest, "handleCancelSpeedtest" | "handleSpeedtest" | "speedtestRunning"> &
  Pick<NodeShared, "nodeGroups" | "t">;
