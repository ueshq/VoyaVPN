import { useProfileActivation } from "@voya/client/runtime-action";
import type { NodeListRow } from "@voya/features/profiles/node-list-rows";
import type { ImportProfilesResult, ProfileSummaryEntry } from "@voya/contracts";
import { profileLatency, profileLatencyTone, profileTitle } from "@voya/features/profiles/profile-display";
import { formatImportSummary } from "@voya/features/profiles/server-table-actions";
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
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { Circle, CircleCheck, ClipboardPaste, Gauge, Rss, RefreshCw, Server } from "lucide-react-native";
import { useCallback, useMemo, useRef, useState } from "react";
import { AccessibilityInfo, findNodeHandle, FlatList, View, useWindowDimensions } from "react-native";
import { useResolveClassNames } from "uniwind";

import { Banner } from "~/components/banner";
import { EmptyState } from "~/components/empty-state";
import { IconBadge } from "~/components/icon-badge";
import { ListCard } from "~/components/list-card";
import { ListRow } from "~/components/list-row";
import { PageHeader } from "~/components/page-header";
import { SectionHeader } from "~/components/section-header";
import { TONE_BACKGROUND, TONE_TEXT, useToneColor, type Tone } from "~/components/tone";
import { useScreenInsets } from "~/components/use-screen-insets";

import { NodeActionsSheet } from "./node-actions-sheet";
import { useNodeSelection } from "@voya/features/profiles/use-node-selection";

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
  const insets = useScreenInsets();
  const accentForeground = useToneColor("brand");
  const { color: onAccent } = useResolveClassNames("text-accent-foreground");
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

  const subscriptions = useNodeSubscriptions(operation, t);
  async function onImported(result: ImportProfilesResult, isActive: () => boolean) {
    if (!isActive()) return;
    operation.setOperationMessage(formatImportSummary(result, t));
    await queryClient.invalidateQueries();
    await subscriptions.updateImportedSubscriptions(result.addedSubscriptionIds, isActive);
    if (isActive()) await queryClient.invalidateQueries();
  }
  const imports = useNodeImport(operation, onImported, t);
  const groups = usePolicyGroups(operation, t);
  // The subscriptions a group could be refreshed from; the same list the
  // policy-group editor offers, so both read one query rather than two.
  const subscriptionRows = groups.policyGroupSubscriptions;
  const exports = useNodeExport(operation, t);
  const [actionsFor, setActionsFor] = useState<ProfileSummaryEntry | null>(null);

  const returnFocusId = useRef<string | null>(null);
  const importRef = useRef<View>(null);
  const nodeRefs = useRef(new Map<string, View>());
  const { width, fontScale } = useWindowDimensions();
  const stackedActions = width / fontScale < 360;

  // A group's first node opens its card; `last` already closes it.
  const rows = useMemo(
    () => data.rows.map((row, index) => ({ first: index === 0 || data.rows[index - 1].kind === "group", row })),
    [data.rows],
  );

  const renderRow = useCallback(
    ({ item: { first, row: item } }: { item: { first: boolean; row: NodeListRow } }) =>
      item.kind === "group" ? (
        <View className="px-page pb-1 pt-4">
          <SectionHeader
            title={item.name}
            detail={t("nodeGroups.membersCount", { count: item.allMembers.length })}
            expanded={item.expanded}
            onToggle={() => selection.toggleGroup(item.groupKey)}
          />
        </View>
      ) : (
        <ListRow
          ref={(node) => {
            if (node) nodeRefs.current.set(item.item.profile.id, node);
            else nodeRefs.current.delete(item.item.profile.id);
          }}
          inset
          first={first}
          last={item.last}
          stacked={stackedActions}
          title={profileTitle(item.item.profile.remarks, t)}
          titleLines={1}
          description={item.item.profile.address}
          leading={
            <SelectionMark
              state={activation.runningId === item.item.profile.id ? "inUse" : item.item.isActive ? "selected" : "none"}
            />
          }
          trailing={
            <View className={`gap-1 ${stackedActions ? "flex-row items-center" : "items-end"}`}>
              {(!item.item.metrics.outcome || item.item.metrics.outcome === "completed") ? (
                <LatencyPill tone={LATENCY_TONE[profileLatencyTone(item.item)]} text={profileLatency(item.item, t)} />
              ) : null}
              {activation.runningId === item.item.profile.id ? (
                <Typography className="text-sm font-medium text-connected">
                  {t("panes.profiles.card.using")}
                </Typography>
              ) : item.item.isActive ? (
                <Typography className="text-sm font-medium text-brand">
                  {t("panes.profiles.card.default")}
                </Typography>
              ) : null}
            </View>
          }
          isDisabled={activation.busy}
          onPress={() => void activation.activateProfile(item.item.profile.id)}
          // A phone has no right-click, so the desktop's row menu is a long
          // press; the sheet says what it offers.
          onLongPress={() => {
            returnFocusId.current = item.item.profile.id;
            setActionsFor(item.item);
          }}
          onAccessibilityAction={(event) => {
            if (event.nativeEvent.actionName === "longpress") {
              returnFocusId.current = item.item.profile.id;
              setActionsFor(item.item);
            }
          }}
          accessibilityActions={[{ label: actionsLabel(item.item, t), name: "longpress" }]}
        >
          {item.item.metrics.outcome && item.item.metrics.outcome !== "completed" ? (
            <Typography className="text-sm text-danger" numberOfLines={2}>{profileLatency(item.item, t)}</Typography>
          ) : null}
        </ListRow>
      ),
    [activation, selection, stackedActions, t],
  );

  const testAllLabel = speedtest.speedtestRunning
    ? speedtest.speedtestProgress
      ? t("panes.profiles.speedtest.stopProgress", speedtest.speedtestProgress)
      : t("panes.profiles.speedtest.stop")
    : t("panes.profiles.speedtest.testAll");

  return (
    <View className="flex-1 bg-canvas">
      <FlatList
        accessibilityElementsHidden={actionsFor !== null || exports.shareQrContent !== null}
        data={rows}
        keyExtractor={({ row }) => row.key}
        renderItem={renderRow}
        contentContainerStyle={insets}
        // The toolbar and subscription controls scroll with rows. A fixed
        // header can consume the entire small-screen viewport at large text.
        keyboardShouldPersistTaps="handled"
        keyboardDismissMode="on-drag"
        ListHeaderComponent={
          <View className="gap-4 px-page">
            <PageHeader
              title={t("tabs.profiles")}
              trailing={
                // One button, two jobs: while a run is in flight it is the way
                // to stop it, and it counts the nodes that have answered.
                <Button
                  className="min-h-10 h-auto rounded-full bg-accent-soft py-2"
                  size="sm"
                  variant="secondary"
                  isDisabled={testableIds.length === 0}
                  onPress={() =>
                    speedtest.speedtestRunning
                      ? void speedtest.handleCancelSpeedtest()
                      : void speedtest.handleSpeedtest({ profileIds: testableIds, scope: "profiles" })
                  }
                >
                  {speedtest.speedtestRunning ? <Spinner size="sm" /> : <Gauge size={16} color={accentForeground} />}
                  <Button.Label>{testAllLabel}</Button.Label>
                </Button>
              }
            />
            <Button
              ref={importRef}
              className="min-h-12 h-auto rounded-full py-3"
              variant="primary"
              isDisabled={imports.directImportPending !== null || subscriptions.updatingSubscriptions.size > 0}
              accessibilityLabel={t("panes.profiles.import.clipboard")}
              onPress={() => void imports.handleDirectImport("clipboard")}
            >
              {imports.directImportPending ? <Spinner size="sm" /> : <ClipboardPaste size={18} color={typeof onAccent === "string" ? onAccent : undefined} />}
              <Button.Label>{t("panes.profiles.import.clipboard")}</Button.Label>
            </Button>
            {operation.operationMessage ? (
              <Banner status="info" liveRegion message={operation.operationMessage} />
            ) : null}
            {operation.operationError ? <Banner status="danger" message={operation.operationError} /> : null}

            {/* A group replaces the single selected node, so its controls precede
                the node rows: picking one is picking *instead* of a row.
                Editing a group is a desktop job; a phone uses what is there. */}
            {groups.policyGroupEntries.length > 0 ? (
              <View>
                <SectionHeader title={t("policyGroups.title")} />
                <ListCard>
                  {groups.policyGroupEntries.map((entry, index, all) => (
                    <ListRow
                      key={entry.group.id}
                      last={index === all.length - 1}
                      title={entry.group.name}
                      titleLines={2}
                      leading={<SelectionMark state={entry.isActive ? (groups.coreConnected ? "inUse" : "selected") : "none"} />}
                      trailing={entry.isActive ? (
                        <Typography className={`text-sm font-medium ${groups.coreConnected ? "text-connected" : "text-brand"}`}>
                          {t(groups.coreConnected ? "policyGroups.inUse" : "policyGroups.active")}
                        </Typography>
                      ) : null}
                      isDisabled={groups.switchingPolicyGroupId !== null}
                      onPress={() => void groups.activatePolicyGroup(entry.group.id)}
                      accessibilityState={{ selected: entry.isActive }}
                    />
                  ))}
                </ListCard>
              </View>
            ) : null}

            {/* Source editing stays on the desktop; imported subscriptions
                can also be refreshed individually or together here. */}
            {subscriptionRows.length > 0 ? (
              <View>
                <SectionHeader
                  title={t("panes.profiles.subscriptionSources")}
                  trailing={
                    <Button
                      className="min-h-10 h-auto py-2"
                      variant="ghost"
                      size="sm"
                      isDisabled={imports.directImportPending !== null || subscriptions.updatingSubscriptions.size > 0}
                      onPress={() => void subscriptions.updateAllSubscriptions()}
                    >
                      <Button.Label className="text-brand">
                        {t("panes.profiles.toolbar.updateAllSubscriptions")}
                      </Button.Label>
                    </Button>
                  }
                />
                <ListCard>
                  {subscriptionRows.map((subscription, index) => {
                    const updating = subscriptions.updatingAllSubscriptions || subscriptions.updatingSubscriptions.has(subscription.id);
                    return (
                      <ListRow
                        key={subscription.id}
                        last={index === subscriptionRows.length - 1}
                        title={subscription.remarks}
                        titleLines={1}
                        leading={<IconBadge icon={Rss} size="sm" />}
                        trailing={
                          <Button
                            isIconOnly
                            className="h-11 w-11"
                            variant="ghost"
                            size="sm"
                            isDisabled={imports.directImportPending !== null || updating}
                            onPress={() => void subscriptions.updateSubscription(subscription.id)}
                            accessibilityLabel={`${t("home.subscriptionCard.update")} ${subscription.remarks}`}
                          >
                            {updating ? <Spinner size="sm" /> : <RefreshCw size={18} color={accentForeground} />}
                          </Button>
                        }
                      />
                    );
                  })}
                </ListCard>
              </View>
            ) : null}
          </View>
        }
        ListEmptyComponent={
          <View className="px-page pt-4">
            <EmptyState
              icons={[Server]}
              title={t("panes.profiles.empty")}
              description={t("panes.profiles.emptyDescription")}
            />
          </View>
        }
      />

      <NodeActionsSheet
        entry={actionsFor}
        exports={exports}
        onClose={() => setActionsFor(null)}
        onClosed={() => {
          const node = returnFocusId.current ? nodeRefs.current.get(returnFocusId.current) : null;
          // Deleting the trigger leaves no row to return to; focus the import
          // button instead of a stale native id.
          const target = findNodeHandle(node ?? importRef.current);
          if (target != null) AccessibilityInfo.setAccessibilityFocus(target);
        }}
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

/** Latency tones: fast is green, slow but reachable is a warning, untested is quiet. */
const LATENCY_TONE = {
  fair: "warning",
  good: "connected",
  poor: "danger",
  unknown: "neutral",
} as const satisfies Record<ReturnType<typeof profileLatencyTone>, Tone>;

function LatencyPill({ text, tone }: { text: string; tone: Tone }) {
  return (
    <View className={`rounded-full px-2.5 py-0.5 ${TONE_BACKGROUND[tone]}`}>
      <Typography className={`text-sm font-medium tabular-nums ${TONE_TEXT[tone]}`}>{text}</Typography>
    </View>
  );
}

/**
 * The radio mark before a choosable row: a tap on the row selects it, and the
 * mark says which one is. Decoration only — the row's own text says it too.
 */
function SelectionMark({ state }: { state: "inUse" | "none" | "selected" }) {
  const tone = state === "inUse" ? "connected" : state === "selected" ? "brand" : "neutral";
  const color = useToneColor(tone);

  return state === "none"
    ? <Circle size={22} color={color} strokeWidth={1.5} accessible={false} />
    : <CircleCheck size={22} color={color} strokeWidth={2} accessible={false} />;
}
