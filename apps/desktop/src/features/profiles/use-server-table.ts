import { useMemo, useRef } from "react";
import { useQuery } from "@tanstack/react-query";

import { queries } from "@voya/client/queries";
import { useI18n } from "@voya/i18n/use-i18n";
import type { ImportProfilesResult, ProfileSummaryEntry } from "@voya/contracts";
import { metadataBySubscriptionId } from "@voya/features/subscriptions/subscription-usage";
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
  // Usage and names are drawn on the desktop's group cards and details dialog
  // only, so they are read here rather than in the list data both apps share.
  const metadataQuery = useQuery(queries.subscriptionMetadata);
  const subscriptionMetadata = useMemo(() => metadataBySubscriptionId(metadataQuery.data ?? []), [metadataQuery.data]);
  const subscriptionNames = useMemo(
    () =>
      new Map(
        (data.subscriptionsQuery.data ?? []).map((item) => [
          item.id,
          item.remarks || t("panes.subscriptions.untitled"),
        ]),
      ),
    [data.subscriptionsQuery.data, t],
  );
  function subscriptionName(item: ProfileSummaryEntry) {
    return item.profile.subscriptionId
      ? (subscriptionNames.get(item.profile.subscriptionId) ?? t("panes.subscriptions.untitled"))
      : t("panes.profiles.card.local");
  }
  const listView = useNodeListVirtual(data.rows, data.search);
  const editor = useNodeEditor(operation, listView.viewportRef, t);
  const activation = useProfileActivation(t);
  // The desktop restores focus to the control that opened a dialog.
  const subscriptions = useNodeSubscriptions<HTMLElement>(operation, t);
  const exports = useNodeExport(operation, t);
  // A subscription URL only creates the source; updating it straight away is
  // what brings its nodes in, so the user never meets an empty group.
  async function handleImported(result: ImportProfilesResult, isActive: () => boolean) {
    await editor.handleDialogImport(result, isActive);
    await subscriptions.updateImportedSubscriptions(result.addedSubscriptionIds, isActive);
  }
  /**
   * The dialog's version: it does not wait for the downloads. They take as
   * long as the network does, and a dialog that has submitted cannot be closed
   * until this returns — so the page banner, which reports the update, would
   * sit behind a modal the user could not dismiss.
   */
  async function handleDialogImported(result: ImportProfilesResult) {
    await editor.handleDialogImport(result);
    void subscriptions.updateImportedSubscriptions(result.addedSubscriptionIds, () => true);
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
    subscriptionMetadata,
    subscriptionName,
    visibleProfileCount: data.rows.reduce((count, row) => count + (row.kind === "group" ? row.members.length : 0), 0),
    ...listView,
    ...editor,
    ...subscriptions,
    ...exports,
    ...speedtest,
    ...imports,
    ...policyGroups,
    handleDialogImported,
  };
}

export type ServerTableController = ReturnType<typeof useServerTable>;
