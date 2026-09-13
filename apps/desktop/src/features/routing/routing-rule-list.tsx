import { useState, type MouseEvent } from "react";
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type Announcements,
  type DragEndEvent,
  type Modifier,
  type UniqueIdentifier,
} from "@dnd-kit/core";
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  AppWindow,
  Globe,
  GripVertical,
  Layers,
  LoaderCircle,
  Network,
  Plug,
  TriangleAlert,
} from "lucide-react";

import type { TranslationFunction } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { Switch } from "@voya/ui/components/switch";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@voya/ui/components/table";
import { cn } from "@voya/ui/lib/utils";

import {
  dataTableHeader,
  dataTableRowEven,
  dataTableRowHover,
  dataTableRowOdd,
} from "@/components/app-shell/data-table-surface";
import type { RoutingRule } from "@/ipc/bindings";

import { OUTBOUND_LABEL_KEYS, describeOutbound, type RuleGroupOutbound } from "./rule-outbound";
import {
  ruleHasMatcher,
  ruleMatchChips,
  type MatchChip,
  type MatchListField,
} from "./rule-match-summary";
import { RULE_SCOPE_LABEL_KEYS } from "./routing-constants";
import { RuleRowContextMenu, RuleRowMenuButton, type RuleMenuActions } from "./routing-rule-menu";
import { ruleDisplayName, sentinelLabelKey } from "./sentinel-rules";
import type { RuleMoveAction } from "./use-routing-screen";

type RoutingRuleListProps = {
  /** Policy groups a rule can target; `null` while the list loads. */
  groupOutbounds?: readonly RuleGroupOutbound[] | null;
  /** Global mode skips every rule, so nothing in the list can change. */
  locked?: boolean;
  /** Node remarks a rule can target; `null` while the node list loads. */
  nodeNames: readonly string[] | null;
  /** Points a rule whose node or group is gone back at the proxy. */
  onFixOutbound?: (rule: RoutingRule) => void;
  onDelete: (rule: RoutingRule) => void;
  onEdit: (rule: RoutingRule) => void;
  onMove: (rule: RoutingRule, action: RuleMoveAction) => void;
  /** Resolves to whether the backend committed the new order. */
  onReorder: (ruleId: string, from: number, to: number) => Promise<boolean>;
  onToggle: (rule: RoutingRule, enabled: boolean) => void;
  /** The state each rule is switching to while its save is in flight. */
  pendingToggles: ReadonlyMap<string, boolean>;
  /** Whether the platform's tunnel can match apps; macOS cannot. */
  processRulesSupported?: boolean;
  rules: readonly RoutingRule[];
};

// Rows only move vertically: sideways pointer travel would drag a row out of
// its table.
const lockHorizontalAxis: Modifier = ({ transform }) => ({ ...transform, x: 0 });
const MODIFIERS = [lockHorizontalAxis];

const LIST_ICONS = { domain: Globe, ip: Network, process: AppWindow, protocol: Layers } as const;
const CHIP_CLASS =
  "inline-flex min-w-0 shrink items-center gap-1 rounded-md border bg-background px-1.5 py-0.5 text-xs";

// Double-clicking a row opens its editor; controls inside the row keep their
// own double-clicks.
function stopDoubleClick(event: MouseEvent) {
  event.stopPropagation();
}

/**
 * The rules of the active rule set in evaluation order. Rows reorder by drag
 * and drop (pointer or keyboard, from the grip handle) or through the row menu,
 * and switch on and off in place.
 */
export function RoutingRuleList({
  groupOutbounds = [],
  locked = false,
  nodeNames,
  onFixOutbound,
  onDelete,
  onEdit,
  onMove,
  onReorder,
  onToggle,
  pendingToggles,
  processRulesSupported = true,
  rules,
}: RoutingRuleListProps) {
  const { t } = useI18n();
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );
  // A dropped row stays where it was dropped during the round trip. The order
  // only applies to the `rules` it was computed from, so it lapses by itself
  // once the committed rule set replaces them.
  const [optimistic, setOptimistic] = useState<{
    base: readonly RoutingRule[];
    order: RoutingRule[];
  } | null>(null);
  const reordering = optimistic !== null && optimistic.base === rules;
  const visible = reordering ? optimistic.order : rules;
  const ids = visible.map((rule) => rule.id);

  function nameOf(id: UniqueIdentifier) {
    const rule = visible.find((item) => item.id === id);
    return rule ? ruleDisplayName(rule, t) : String(id);
  }

  function positionOf(id: UniqueIdentifier) {
    return ids.indexOf(String(id)) + 1;
  }

  const announcements: Announcements = {
    onDragCancel: ({ active }) => t("panes.routing.dragCancel", { name: nameOf(active.id) }),
    onDragEnd: ({ active, over }) =>
      over
        ? t("panes.routing.dragEnd", { name: nameOf(active.id), position: positionOf(over.id) })
        : undefined,
    onDragOver: ({ active, over }) =>
      over
        ? t("panes.routing.dragOver", { name: nameOf(active.id), position: positionOf(over.id) })
        : undefined,
    onDragStart: ({ active }) => t("panes.routing.dragStart", { name: nameOf(active.id) }),
  };

  function handleDragEnd({ active, over }: DragEndEvent) {
    const from = ids.indexOf(String(active.id));
    const to = over ? ids.indexOf(String(over.id)) : -1;
    if (from < 0 || to < 0 || from === to) {
      return;
    }
    setOptimistic({ base: rules, order: arrayMove([...visible], from, to) });
    void onReorder(String(active.id), from, to).then((committed) => {
      if (!committed) {
        setOptimistic(null);
      }
    });
  }

  return (
    <DndContext
      accessibility={{
        announcements,
        screenReaderInstructions: { draggable: t("panes.routing.dragInstructions") },
      }}
      collisionDetection={closestCenter}
      modifiers={MODIFIERS}
      onDragEnd={handleDragEnd}
      sensors={sensors}
    >
      <Table className="min-w-[40rem] table-fixed">
        <TableHeader className={cn("sticky top-0 z-10 [&_tr]:border-0", dataTableHeader)}>
          <TableRow className="hover:bg-transparent">
            <TableHead className="w-10 px-2" scope="col">
              <span className="sr-only">{t("panes.routing.columnOrder")}</span>
            </TableHead>
            <TableHead className="w-12 px-2" scope="col">
              <span className="sr-only">{t("panes.routing.enabled")}</span>
            </TableHead>
            <TableHead className="w-[30%] px-3 text-muted-foreground" scope="col">
              {t("panes.routing.columnName")}
            </TableHead>
            <TableHead className="px-3 text-muted-foreground" scope="col">
              {t("panes.routing.columnMatch")}
            </TableHead>
            <TableHead className="w-32 px-3 text-muted-foreground" scope="col">
              {t("panes.routing.outbound")}
            </TableHead>
            <TableHead className="w-11 px-2" scope="col">
              <span className="sr-only">{t("panes.routing.columnActions")}</span>
            </TableHead>
          </TableRow>
        </TableHeader>
        <SortableContext items={ids} strategy={verticalListSortingStrategy}>
          <TableBody>
            {visible.map((rule, index) => (
              <SortableRuleRow
                canMoveDown={!locked && !reordering && index < visible.length - 1}
                canMoveUp={!locked && !reordering && index > 0}
                groupOutbounds={groupOutbounds}
                key={rule.id}
                locked={locked}
                nodeNames={nodeNames}
                onFixOutbound={onFixOutbound}
                onDelete={onDelete}
                onEdit={onEdit}
                onMove={onMove}
                onToggle={onToggle}
                pendingEnabled={pendingToggles.get(rule.id)}
                processRulesSupported={processRulesSupported}
                reordering={reordering}
                rule={rule}
                striped={index % 2 === 0}
              />
            ))}
          </TableBody>
        </SortableContext>
      </Table>
    </DndContext>
  );
}

function SortableRuleRow({
  canMoveDown,
  canMoveUp,
  locked,
  nodeNames,
  onDelete,
  onEdit,
  onMove,
  onToggle,
  pendingEnabled,
  processRulesSupported,
  reordering,
  rule,
  striped,
  groupOutbounds,
  onFixOutbound,
}: Pick<
  RoutingRuleListProps,
  "groupOutbounds" | "nodeNames" | "onDelete" | "onEdit" | "onFixOutbound" | "onMove" | "onToggle"
> & {
  canMoveDown: boolean;
  canMoveUp: boolean;
  locked: boolean;
  pendingEnabled: boolean | undefined;
  processRulesSupported: boolean;
  reordering: boolean;
  rule: RoutingRule;
  striped: boolean;
}) {
  const { t } = useI18n();
  const name = ruleDisplayName(rule, t);
  const {
    attributes,
    isDragging,
    listeners,
    setActivatorNodeRef,
    setNodeRef,
    transform,
    transition,
  } = useSortable({
    attributes: { roleDescription: t("panes.routing.sortableRole") },
    // One move at a time: indices of a second drag would be computed against
    // an order the backend has not committed yet. Locked rules do not move.
    disabled: reordering || locked,
    id: rule.id,
  });
  const enabled = pendingEnabled ?? rule.enabled;
  const muted = enabled && !locked ? undefined : "opacity-55";
  const menuLabel = t("panes.routing.ruleActions", { name });
  const actions: RuleMenuActions = {
    canMoveDown,
    canMoveUp,
    locked,
    onDelete: () => onDelete(rule),
    onEdit: () => onEdit(rule),
    onMove: (action) => onMove(rule, action),
  };

  return (
    <RuleRowContextMenu actions={actions} label={menuLabel}>
      <TableRow
        className={cn(
          "border-0",
          striped ? dataTableRowEven : dataTableRowOdd,
          dataTableRowHover,
          isDragging && "relative z-20 bg-surface-raised shadow-md",
        )}
        onDoubleClick={locked ? undefined : () => onEdit(rule)}
        ref={setNodeRef}
        style={{ transform: CSS.Translate.toString(transform), transition }}
      >
        <TableCell className="px-2 py-1.5" onDoubleClick={stopDoubleClick}>
          <button
            {...attributes}
            {...listeners}
            aria-label={t("panes.routing.dragHandle", { name })}
            className="grid size-7 cursor-grab touch-none place-items-center rounded-md text-muted-foreground outline-none hover:bg-surface-hovered focus-visible:ring-2 focus-visible:ring-ring active:cursor-grabbing aria-disabled:cursor-not-allowed aria-disabled:opacity-50"
            ref={setActivatorNodeRef}
            type="button"
          >
            <GripVertical aria-hidden="true" className="size-4" />
          </button>
        </TableCell>
        <TableCell className="px-2 py-1.5" onDoubleClick={stopDoubleClick}>
          <span className="flex items-center gap-1.5">
            <Switch
              aria-busy={pendingEnabled !== undefined}
              aria-label={t("panes.routing.toggleRule", { name })}
              checked={enabled}
              disabled={locked || pendingEnabled !== undefined}
              onCheckedChange={(checked) => onToggle(rule, checked)}
            />
            {pendingEnabled !== undefined ? (
              <LoaderCircle aria-hidden="true" className="size-3.5 animate-spin text-muted-foreground" />
            ) : null}
          </span>
        </TableCell>
        <TableCell className={cn("overflow-hidden px-3 py-1.5", muted)}>
          <span className="flex min-w-0 items-center gap-2">
            <span className="truncate font-medium" title={name}>
              {name}
            </span>
            {sentinelLabelKey(rule.remarks) ? (
              <Badge
                className="shrink-0 bg-background"
                title={t("panes.routing.managedRuleHint")}
                variant="outline"
              >
                {t("panes.routing.managedRule")}
              </Badge>
            ) : null}
          </span>
        </TableCell>
        <TableCell className={cn("overflow-hidden px-3 py-1.5", muted)}>
          <RuleMatch processRulesSupported={processRulesSupported} rule={rule} />
        </TableCell>
        <TableCell className={cn("overflow-hidden px-3 py-1.5", muted)}>
          <OutboundBadge
            groupOutbounds={groupOutbounds}
            locked={locked}
            nodeNames={nodeNames}
            onFix={onFixOutbound ? () => onFixOutbound(rule) : undefined}
            outbound={rule.outbound}
          />
        </TableCell>
        <TableCell className="px-2 py-1.5" onDoubleClick={stopDoubleClick}>
          <RuleRowMenuButton actions={actions} label={menuLabel} />
        </TableCell>
      </TableRow>
    </RuleRowContextMenu>
  );
}

function RuleMatch({
  processRulesSupported,
  rule,
}: {
  processRulesSupported: boolean;
  rule: RoutingRule;
}) {
  const { t } = useI18n();
  if (!ruleHasMatcher(rule)) {
    return (
      <span className="flex min-w-0 items-center gap-1 text-xs text-warning">
        <TriangleAlert aria-hidden="true" className="size-3.5 shrink-0" />
        <span className="truncate">{t("panes.routing.matchNothing")}</span>
      </span>
    );
  }

  return (
    <span className="flex min-w-0 items-center gap-1.5">
      {ruleMatchChips(rule).map((chip) => (
        <MatchChipView
          chip={chip}
          key={chip.kind === "list" ? chip.field : chip.kind}
          processRulesSupported={processRulesSupported}
        />
      ))}
    </span>
  );
}

function MatchChipView({
  chip,
  processRulesSupported,
}: {
  chip: MatchChip;
  processRulesSupported: boolean;
}) {
  const { t } = useI18n();
  switch (chip.kind) {
    case "scope":
      return (
        <span className={cn(CHIP_CLASS, "text-muted-foreground")}>
          {t(RULE_SCOPE_LABEL_KEYS[chip.scope])}
        </span>
      );
    case "port":
      return (
        <span
          className={CHIP_CLASS}
          title={chip.port ? t("panes.routing.port") : t("panes.routing.network")}
        >
          <Plug aria-hidden="true" className="size-3 shrink-0" />
          <span className="truncate">{[chip.network, chip.port].filter(Boolean).join(" ")}</span>
        </span>
      );
    case "list": {
      const Icon = LIST_ICONS[chip.field];
      // A stored app condition is skipped where the tunnel cannot match apps.
      const unsupported = chip.field === "process" && !processRulesSupported;
      return (
        <span
          className={cn(CHIP_CLASS, unsupported && "text-warning line-through")}
          title={
            unsupported
              ? t("panes.routing.processUnsupported")
              : matchFieldTitle(chip.field, t)
          }
        >
          <Icon aria-hidden="true" className="size-3 shrink-0" />
          <span className="truncate">{chip.first}</span>
          {chip.more > 0 ? (
            <span className="shrink-0 text-muted-foreground">
              {t("panes.routing.matchMore", { count: chip.more })}
            </span>
          ) : null}
        </span>
      );
    }
  }
}

function matchFieldTitle(field: MatchListField, t: TranslationFunction) {
  switch (field) {
    case "domain":
      return t("panes.routing.domain");
    case "ip":
      return "IP";
    case "process":
      return t("panes.routing.process");
    case "protocol":
      return t("panes.routing.protocol");
  }
}

function OutboundBadge({
  groupOutbounds,
  locked,
  nodeNames,
  onFix,
  outbound,
}: Pick<RoutingRuleListProps, "groupOutbounds" | "nodeNames"> & {
  locked: boolean;
  onFix?: () => void;
  outbound: string | null;
}) {
  const { t } = useI18n();
  const target = describeOutbound(outbound, nodeNames, groupOutbounds);
  switch (target.kind) {
    case "missing":
    case "missingGroup":
      return (
        <span className="flex min-w-0 flex-col items-start gap-1">
          <Badge className="max-w-full bg-warning-bg text-warning" variant="outline">
            <TriangleAlert aria-hidden="true" />
            <span className="truncate">
              {target.kind === "missingGroup"
                ? t("panes.routing.outboundGroupMissing")
                : t("panes.routing.outboundMissing", { name: target.name })}
            </span>
          </Badge>
          {onFix ? (
            <Button
              className="h-6 px-2 text-xs"
              disabled={locked}
              onClick={onFix}
              size="sm"
              type="button"
              variant="ghost"
            >
              {t("panes.routing.fixOutbound")}
            </Button>
          ) : null}
        </span>
      );
    case "group":
      return (
        <Badge className="max-w-full bg-background" title={target.name} variant="outline">
          <Layers aria-hidden="true" />
          <span className="truncate">{target.name}</span>
        </Badge>
      );
    case "node":
      return (
        <Badge className="max-w-full bg-background" title={target.name} variant="outline">
          <span className="truncate">{target.name}</span>
        </Badge>
      );
    default:
      return (
        <Badge
          className={cn(
            "bg-background",
            target.kind === "block" && "border-danger-border text-danger",
          )}
          variant="outline"
        >
          {t(OUTBOUND_LABEL_KEYS[target.kind])}
        </Badge>
      );
  }
}
