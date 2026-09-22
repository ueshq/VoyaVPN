import { useProfileActivation } from "@voya/client/runtime-action";
import type { NodeListRow } from "@voya/features/profiles/node-list-rows";
import type { ProfileSummaryEntry } from "@voya/contracts";
import { profileLatency, profileTitle } from "@voya/features/profiles/profile-display";
import { useNodeImport } from "@voya/features/profiles/use-node-import";
import { useNodeListData } from "@voya/features/profiles/use-node-list-data";
import { useNodeOperation } from "@voya/features/profiles/use-node-operation";
import { useNodeSpeedtest } from "@voya/features/profiles/use-node-speedtest";
import { useNodeExport } from "@voya/features/profiles/use-node-export";
import { useNodeSubscriptions } from "@voya/features/profiles/use-node-subscriptions";
import { usePolicyGroups } from "@voya/features/profiles/use-policy-groups";
import { useI18n } from "@voya/i18n/use-i18n";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "heroui-native/button";
import { Input } from "heroui-native/input";
import { PressableFeedback } from "heroui-native/pressable-feedback";
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { useCallback, useMemo, useState } from "react";
import { FlatList, View } from "react-native";

import { NodeActionsSheet } from "./node-actions-sheet";
import { useNodeSelection } from "./use-node-selection";

/**
 * The node list.
 *
 * The rows come from `nodeListRows` through `useNodeListData` — the same
 * shaping the desktop table uses, group headers included — so a list that is
 * grouped, filtered and sorted one way there is grouped, filtered and sorted
 * the same way here. A `FlatList` renders them directly rather than a
 * `SectionList` re-deriving sections the shaping already decided.
 */
export function NodesScreen() {
  const { t } = useI18n();
  const selection = useNodeSelection();
  const data = useNodeListData(selection, t);
  const operation = useNodeOperation();
  const speedtest = useNodeSpeedtest(operation);
  const activation = useProfileActivation(t);
  const queryClient = useQueryClient();

  // Groups are headers, not nodes; a run tests what the list is showing.
  const testableIds = useMemo(
    () => data.rows.flatMap((row) => (row.kind === "group" ? [] : [row.item.profile.id])),
    [data.rows],
  );

  const onImported = useCallback(async () => {
    await queryClient.invalidateQueries();
  }, [queryClient]);
  const imports = useNodeImport(operation, onImported, t);
  const groups = usePolicyGroups(operation, t);
  const subscriptions = useNodeSubscriptions(operation, t);
  // The subscriptions a group could be refreshed from; the same list the
  // policy-group editor offers, so both read one query rather than two.
  const subscriptionRows = groups.policyGroupSubscriptions;
  const exports = useNodeExport(operation, t);
  const [actionsFor, setActionsFor] = useState<ProfileSummaryEntry | null>(null);

  const renderRow = useCallback(
    ({ item }: { item: NodeListRow }) =>
      item.kind === "group" ? (
        <PressableFeedback
          animation="disable-all"
          className="flex-row items-center justify-between bg-canvas px-4 py-2"
          onPress={() => selection.toggleGroup(item.groupKey)}
          accessibilityRole="button"
        >
          <PressableFeedback.Highlight />
          <Typography className="text-caption font-medium uppercase text-subtle">
            {item.name}
          </Typography>
          <Typography className="text-caption text-subtlest">
            {t("nodeGroups.membersCount", { count: item.allMembers.length })}
          </Typography>
        </PressableFeedback>
      ) : (
        <PressableFeedback
          animation="disable-all"
          className="flex-row items-center justify-between border-b border-border-subtle bg-surface px-4 py-3"
          isDisabled={activation.busy}
          onPress={() => void activation.activateProfile(item.item.profile.id)}
          // A phone has no right-click, so the desktop's row menu is a long
          // press; the sheet says what it offers.
          onLongPress={() => setActionsFor(item.item)}
          accessibilityRole="button"
          accessibilityActions={[{ label: actionsLabel(item.item, t), name: "longpress" }]}
        >
          <PressableFeedback.Highlight />
          <View className="flex-1 gap-0.5 pr-3">
            <Typography className="text-body text-foreground" numberOfLines={1}>
              {profileTitle(item.item.profile.remarks, t)}
            </Typography>
            <Typography className="text-caption text-subtlest" numberOfLines={1}>
              {item.item.profile.address}
            </Typography>
          </View>
          <View className="items-end gap-0.5">
            <Typography className="text-caption text-subtle">
              {profileLatency(item.item, t)}
            </Typography>
            {activation.runningId === item.item.profile.id ? (
              <Typography className="text-caption text-connected">
                {t("panes.profiles.card.using")}
              </Typography>
            ) : item.item.isActive ? (
              <Typography className="text-caption text-brand">
                {t("panes.profiles.card.default")}
              </Typography>
            ) : null}
          </View>
        </PressableFeedback>
      ),
    [activation, selection, t],
  );

  return (
    <View className="flex-1 bg-canvas">
      <View className="gap-2 p-page">
        <Input
          placeholder={t("panes.profiles.search.placeholder")}
          accessibilityLabel={t("panes.profiles.search.placeholder")}
          value={selection.search}
          onChangeText={selection.setSearch}
          autoCapitalize="none"
          autoCorrect={false}
        />
        <View className="flex-row gap-2">
          <Button
            className="flex-1"
            variant="outline"
            isDisabled={imports.directImportPending !== null}
            onPress={() => void imports.handleDirectImport("clipboard")}
          >
            {imports.directImportPending ? <Spinner size="sm" /> : null}
            <Button.Label>{t("panes.profiles.import.clipboard")}</Button.Label>
          </Button>
          {/* One button, two jobs: while a run is in flight it is the way to
              stop it, and it counts the nodes that have answered. */}
          <Button
            className="flex-1"
            variant="outline"
            isDisabled={testableIds.length === 0}
            onPress={() =>
              speedtest.speedtestRunning
                ? void speedtest.handleCancelSpeedtest()
                : void speedtest.handleSpeedtest({ profileIds: testableIds, scope: "profiles" })
            }
          >
            {speedtest.speedtestRunning ? <Spinner size="sm" /> : null}
            <Button.Label>
              {speedtest.speedtestRunning
                ? speedtest.speedtestProgress
                  ? t("panes.profiles.speedtest.stopProgress", speedtest.speedtestProgress)
                  : t("panes.profiles.speedtest.stop")
                : t("panes.profiles.speedtest.testAll")}
            </Button.Label>
          </Button>
        </View>
        {operation.operationError ? (
          <Typography className="text-caption text-danger">{operation.operationError}</Typography>
        ) : null}

        {/* A group replaces the single selected node, so it sits above the
            list rather than in it: picking one is picking *instead* of a row.
            Editing a group is a desktop job; a phone uses what is there. */}
        {groups.policyGroupEntries.length > 0 ? (
          <View className="gap-2">
            <Typography className="text-caption uppercase text-subtle">
              {t("policyGroups.title")}
            </Typography>
            <View className="flex-row flex-wrap gap-2">
              {groups.policyGroupEntries.map((entry) => (
                <PressableFeedback
                  key={entry.group.id}
                  className={`rounded-2xl border px-3 py-2 ${
                    entry.isActive ? "border-brand bg-brand-tint" : "border-border bg-surface"
                  }`}
                  isDisabled={groups.switchingPolicyGroupId !== null}
                  onPress={() => void groups.activatePolicyGroup(entry.group.id)}
                  accessibilityRole="button"
                  accessibilityState={{ selected: entry.isActive }}
                >
                  <PressableFeedback.Highlight />
                  <Typography className="text-body text-foreground">{entry.group.name}</Typography>
                  {entry.isActive ? (
                    <Typography className="text-caption text-brand">
                      {t(groups.coreConnected ? "policyGroups.inUse" : "policyGroups.active")}
                    </Typography>
                  ) : null}
                </PressableFeedback>
              ))}
            </View>
          </View>
        ) : null}

        {/* Refreshing a source, not managing one: adding and editing
            subscriptions stays on the desktop, where the form belongs. */}
        {subscriptionRows.length > 0 ? (
          <View className="gap-2">
            <View className="flex-row items-center justify-between">
              <Typography className="text-caption uppercase text-subtle">
                {t("panes.profiles.subscriptionSources")}
              </Typography>
              <Button
                variant="ghost"
                size="sm"
                isDisabled={subscriptions.updatingAllSubscriptions}
                onPress={() => void subscriptions.updateAllSubscriptions()}
              >
                <Button.Label>
                  {t("panes.profiles.toolbar.updateAllSubscriptions")}
                </Button.Label>
              </Button>
            </View>
            {subscriptionRows.map((subscription) => (
              <View
                key={subscription.id}
                className="flex-row items-center justify-between rounded-2xl bg-surface px-3 py-2"
              >
                <Typography
                  className="flex-1 pr-3 text-body text-foreground"
                  numberOfLines={1}
                >
                  {subscription.remarks}
                </Typography>
                <Button
                  variant="ghost"
                  size="sm"
                  isDisabled={subscriptions.updatingSubscriptions.has(subscription.id)}
                  onPress={() => void subscriptions.updateSubscription(subscription.id)}
                  accessibilityLabel={`${t("home.subscriptionCard.update")} ${subscription.remarks}`}
                >
                  <Button.Label>{t("home.subscriptionCard.update")}</Button.Label>
                </Button>
              </View>
            ))}
          </View>
        ) : null}
      </View>

      <FlatList
        data={data.rows}
        keyExtractor={(row) => row.key}
        renderItem={renderRow}
        ListEmptyComponent={
          <View className="items-center gap-1 p-page">
            <Typography className="text-body text-foreground">
              {selection.search ? t("panes.profiles.search.empty") : t("panes.profiles.empty")}
            </Typography>
            <Typography className="text-caption text-subtle">
              {selection.search
                ? t("panes.profiles.search.emptyHint")
                : t("panes.profiles.emptyDescription")}
            </Typography>
          </View>
        }
      />

      <NodeActionsSheet
        entry={actionsFor}
        exports={exports}
        onClose={() => setActionsFor(null)}
        operation={operation}
      />
    </View>
  );
}

/** What the long press does, for a screen reader that cannot long-press. */
function actionsLabel(entry: ProfileSummaryEntry, t: ReturnType<typeof useI18n>["t"]) {
  return t("panes.profiles.menu.actionsFor", {
    name: profileTitle(entry.profile.remarks, t),
  });
}
