import { ErrorNotice } from "~/components/error-notice";
import { openPage } from "~/app/navigation";
import { MoreHorizontal } from "lucide-react-native";
import { useProfileActivation } from "@voya/client/runtime-action";
import type { NodeListRow } from "@voya/features/profiles/node-list-rows";
import type { ProfileSummaryEntry } from "@voya/contracts";
import { profileLatency, profileLatencyFailed, profileLatencyTone, profileTitle } from "@voya/features/profiles/profile-display";
import { overlaySpeedtestResult, useNodeListData } from "@voya/features/profiles/use-node-list-data";
import { useNodeOperation } from "@voya/features/profiles/use-node-operation";
import { useNodeSpeedtest } from "@voya/features/profiles/use-node-speedtest";
import { useNodeExport } from "@voya/features/profiles/use-node-export";
import { POLICY_GROUP_STRATEGY_HINT_KEYS } from "@voya/features/profiles/policy-group-labels";
import { usePolicyGroups } from "@voya/features/profiles/use-policy-groups";
import type { TranslationFunction } from "@voya/i18n/core";
import { useI18n } from "@voya/i18n/use-i18n";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { getProtocolLabel } from "@voya/features/profiles/profile-constants";
import { Button } from "heroui-native/button";
import { Chip } from "heroui-native/chip";
import { ListGroup } from "heroui-native/list-group";
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { ClipboardPaste, Gauge, Server } from "lucide-react-native";
import { useLatestRef } from "@voya/utils/use-latest-ref";
import { type RefObject, memo, useCallback, useMemo, useRef, useState } from "react";
import { AccessibilityInfo, findNodeHandle, FlatList, View, useWindowDimensions } from "react-native";

import { Banner } from "~/components/banner";
import { Disclosure } from "~/components/disclosure";
import { EmptyState } from "~/components/empty-state";
import { ListRow } from "~/components/list-row";
import { SelectionMark } from "~/components/selection-mark";
import { PageHeader } from "~/components/page-header";
import { SectionHeader } from "~/components/section-header";
import { useClassColor, useToneColor } from "~/components/tone";
import { useScreenInsets } from "~/components/use-screen-insets";

import { NodeActionsSheet } from "./node-actions-sheet";
import { NodeSortMenu } from "./node-sort-sheet";
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
  const onAccent = useClassColor("text-accent-foreground");
  const selection = useNodeSelection();
  const data = useNodeListData(selection, t);
  const operation = useNodeOperation();
  const speedtest = useNodeSpeedtest(operation);
  const activation = useProfileActivation(t);
  // The hooks above hand back a fresh object every render. A row renderer
  // that depended on them would be rebuilt every render too, and with it
  // every visible row — on each store push, for a list that can hold
  // thousands. So it depends on the fields it reads, and reaches the one
  // callback that is rebuilt each render through a ref.
  const { busy: switching, runningId } = activation;
  const { toggleGroup } = selection;
  const selectRef = useLatestRef((id: string) =>
    operation.runOperation(() => activation.selectProfile(id)),
  );

  // A run tests every node the list holds. Collapsing a group only hides its
  // rows — it must not take the nodes out of a "Test all" — and a search that
  // narrows the view is no reason to test less than everything either.
  // The stored list, not the one with live results laid over it: the overlay
  // is a new array on every result of a run and holds the same nodes.
  const storedEntries = data.profilesQuery.data?.entries;
  const testableIds = useMemo(
    () => (storedEntries ?? []).map((entry) => entry.profile.id),
    [storedEntries],
  );

  const selected = data.profiles.find((entry) => entry.isActive);
  // The phone lists the groups without their running member or its delays,
  // so the every-three-seconds read behind those is left off.
  const groups = usePolicyGroups(operation, t, { live: false });
  const exports = useNodeExport(operation, t);
  const [actionsFor, setActionsFor] = useState<ProfileSummaryEntry | null>(null);

  const returnFocusId = useRef<string | null>(null);
  // Focus goes back to the row the sheet was opened from once it closes.
  const openActions = useCallback((entry: ProfileSummaryEntry) => {
    returnFocusId.current = entry.profile.id;
    setActionsFor(entry);
  }, [setActionsFor]);
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
            onToggle={() => toggleGroup(item.groupKey)}
          />
        </View>
      ) : (
        <NodeRow
          stored={item.item}
          first={first}
          last={item.last}
          runningId={runningId}
          switching={switching}
          stackedActions={stackedActions}
          t={t}
          accentForeground={accentForeground}
          selectRef={selectRef}
          openActions={openActions}
          nodeRefs={nodeRefs}
        />
      ),
    [runningId, switching, toggleGroup, stackedActions, t, accentForeground, selectRef, openActions],
  );

  const testAllLabel = speedtest.speedtestRunning
    ? speedtest.speedtestProgress
      ? t("panes.profiles.speedtest.stopProgress", speedtest.speedtestProgress)
      : t("panes.profiles.speedtest.stop")
    : t("panes.profiles.speedtest.testAll");

  return (
    <>
      <FlatList
        className="flex-1 bg-canvas"
        accessibilityElementsHidden={actionsFor !== null || exports.shareQrContent !== null}
        data={rows}
        keyExtractor={({ row }) => row.key}
        renderItem={renderRow}
        contentContainerStyle={insets}
        // The header controls scroll with rows. A fixed header can consume
        // the entire small-screen viewport at large text.
        ListHeaderComponent={
          <View className="gap-4 px-page">
            <PageHeader
              title={t("tabs.profiles")}
              trailing={
                <View className="flex-row items-center gap-2">
                  <NodeSortMenu
                    color={accentForeground}
                    sortByLatency={selection.sortByLatency}
                    setSortByLatency={selection.setSortByLatency}
                  />
                  {/* One button, two jobs: while a run is in flight it is the way
                      to stop it, and it counts the nodes that have answered. */}
                  <Button
                    className="min-h-12 h-auto py-2"
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
                </View>
              }
            />
            {selected ? <ListGroup><ListRow
              title={`${t("mobile.currentSelection")}: ${profileTitle(selected.profile.remarks, t)}`}
              titleLines={0} description={selected.profile.address} last
              onPress={() => openActions(selected)}
            /></ListGroup> : null}
            <Button
              ref={importRef}
              className="py-3"
              variant={testableIds.length ? "secondary" : "primary"}
              accessibilityLabel={t("mobile.add")}
              onPress={() => openPage("import")}
            >
              <ClipboardPaste size={18} color={testableIds.length ? accentForeground : onAccent} />
              <Button.Label>{t("mobile.add")}</Button.Label>
            </Button>
            <ListGroup><ListRow last chevron title={t("mobile.subscriptions")} onPress={() => openPage("subscriptions")} /></ListGroup>
            {operation.operationMessage ? (
              <Banner status="info" liveRegion message={operation.operationMessage} />
            ) : null}
            <ErrorNotice error={operation.operationError} />
            <ErrorNotice error={data.profilesQuery.error} retry={() => void data.profilesQuery.refetch()} />

            {/* A group replaces the single selected node, so its controls precede
                the node rows: picking one is picking *instead* of a row.
                Editing a group is a desktop job; a phone uses what is there. */}
            {groups.policyGroupEntries.length > 0 ? (
              <Disclosure heading title={t("policyGroups.title")}>
                <ListGroup>
                  {groups.policyGroupEntries.map((entry, index, all) => (
                    <ListRow
                      key={entry.group.id}
                      last={index === all.length - 1}
                      title={entry.group.name}
                      titleLines={2}
                      description={t(POLICY_GROUP_STRATEGY_HINT_KEYS[entry.group.strategy])}
                      descriptionLines={0}
                      leading={<SelectionMark state={entry.isActive ? (groups.coreConnected ? "inUse" : "selected") : "none"} />}
                      trailing={entry.isActive ? (
                        <Typography className={`text-sm font-medium ${groups.coreConnected ? "text-connected" : "text-brand"}`}>
                          {t(groups.coreConnected ? "policyGroups.inUse" : "policyGroups.active")}
                        </Typography>
                      ) : null}
                      isDisabled={groups.switchingPolicyGroupId !== null}
                      // Like a node row: a stopped core only remembers the
                      // group; connecting stays the Home button's job.
                      onPress={() =>
                        void operation.runOperation(() => groups.selectPolicyGroup(entry.group.id))
                      }
                      accessibilityState={{ selected: entry.isActive }}
                    />
                  ))}
                </ListGroup>
              </Disclosure>
            ) : null}
          </View>
        }
        ListEmptyComponent={data.profilesQuery.isPending ? <Typography className="px-page py-4 text-base text-subtle">{t("panes.profiles.loadingNodes")}</Typography> : data.profilesQuery.error ? undefined :
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
        onTest={(id) => void speedtest.handleSpeedtest({ profileIds: [id], scope: "profiles" })}
      />
    </>
  );
}

/** What the long press does, for a screen reader that cannot long-press. */
function actionsLabel(entry: ProfileSummaryEntry, t: ReturnType<typeof useI18n>["t"]) {
  return t("panes.profiles.menu.actionsFor", {
    name: profileTitle(entry.profile.remarks, t),
  });
}

/** Latency colours: fast is green, slow but reachable is a warning, untested is quiet. */
const LATENCY_COLOR = {
  fair: "warning",
  good: "success",
  poor: "danger",
  unknown: "default",
} as const satisfies Record<ReturnType<typeof profileLatencyTone>, "danger" | "default" | "success" | "warning">;

/**
 * One node's row.
 *
 * A component rather than part of the list's renderer so that it can read its
 * own speedtest result: the list is laid out from the stored listing, which
 * only changes when a run's results are read back, and a row that waited for
 * that showed neither "testing" nor a delay until seconds after it was known.
 * Reading its one entry of the store, a row redraws when its node's result
 * arrives and at no other row's.
 */
const NodeRow = memo(function NodeRow({
  stored,
  first,
  last,
  runningId,
  switching,
  stackedActions,
  t,
  accentForeground,
  selectRef,
  openActions,
  nodeRefs,
}: {
  stored: ProfileSummaryEntry;
  first: boolean;
  last: boolean;
  runningId: string | null;
  switching: boolean;
  stackedActions: boolean;
  t: TranslationFunction;
  accentForeground: string | undefined;
  selectRef: RefObject<(id: string) => unknown>;
  openActions: (entry: ProfileSummaryEntry) => void;
  nodeRefs: RefObject<Map<string, View>>;
}) {
  const result = useRuntimeEventStore(
    (state) => state.speedtestResultsByProfileId[stored.profile.id],
  );
  const entry = overlaySpeedtestResult(stored, result);
  // The pill always holds the latency column's place. A failure's reason is
  // too long for it, so the pill says it failed and the reason gets a line.
  const failed = profileLatencyFailed(entry);
  const latencyText = profileLatency(entry, t);

  return (
    <ListRow
      ref={(node) => {
        if (node) nodeRefs.current.set(entry.profile.id, node);
        else nodeRefs.current.delete(entry.profile.id);
      }}
      inset
      first={first}
      last={last}
      stacked={stackedActions}
      title={profileTitle(entry.profile.remarks, t)}
      titleLines={stackedActions ? 0 : 2}
      description={`${getProtocolLabel(entry.profile.kind)} · ${entry.profile.address}`}
      leading={
        <SelectionMark
          state={runningId === entry.profile.id ? "inUse" : entry.isActive ? "selected" : "none"}
        />
      }
      trailingInteractive
      accessibilityLabel={`${profileTitle(entry.profile.remarks, t)}, ${getProtocolLabel(entry.profile.kind)}, ${entry.profile.address}, ${latencyText}`}
      trailing={
        <View className={`gap-1 ${stackedActions ? "flex-row items-center" : "items-end"}`}>
          <Button isIconOnly className="h-12 w-12" variant="ghost" accessibilityLabel={actionsLabel(entry, t)} onPress={() => openActions(entry)}>
            <MoreHorizontal size={20} color={accentForeground} />
          </Button>
          <Chip size="sm" variant="soft" color={LATENCY_COLOR[profileLatencyTone(entry)]}>
            <Chip.Label className="tabular-nums">
              {failed ? t("speedtest.outcome.failed") : latencyText}
            </Chip.Label>
          </Chip>
          {runningId === entry.profile.id ? (
            <Typography className="text-sm font-medium text-connected">
              {t("panes.profiles.card.using")}
            </Typography>
          ) : entry.isActive ? (
            <Typography className="text-sm font-medium text-brand">
              {t("panes.profiles.card.default")}
            </Typography>
          ) : null}
        </View>
      }
      accessibilityState={{ selected: entry.isActive, busy: switching }}
      isDisabled={switching}
      // Tapping the already-active row selects nothing new — it opens the
      // actions sheet instead, the same thing the header's
      // current-selection row does, so the tap always answers.
      onPress={() =>
        entry.isActive
          ? openActions(entry)
          : void selectRef.current(entry.profile.id)
      }
      // A phone has no right-click, so the desktop's row menu is a long
      // press; the sheet says what it offers.
      onLongPress={() => openActions(entry)}
      onAccessibilityAction={(event) => {
        if (event.nativeEvent.actionName === "longpress") openActions(entry);
      }}
      accessibilityActions={[{ label: actionsLabel(entry, t), name: "longpress" }]}
    >
      {failed && entry.metrics.outcome !== "failed" ? (
        <Typography className="text-sm text-danger" numberOfLines={2}>{latencyText}</Typography>
      ) : null}
    </ListRow>
  );
});
