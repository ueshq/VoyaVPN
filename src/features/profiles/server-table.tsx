import { useMemo, useRef, useState } from "react";
import type * as React from "react";
import * as ContextMenu from "@radix-ui/react-context-menu";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getCoreRowModel, useReactTable, type ColumnDef } from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Check,
  ChevronsDown,
  ChevronsUp,
  Clock,
  Copy,
  FilePlus2,
  Filter,
  Gauge,
  Pencil,
  Play,
  Radio,
  RefreshCw,
  Rows3,
  Rss,
  Search,
  Square,
  Trash2,
  Upload,
  Wifi,
  Zap,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  copyProfiles,
  cancelSpeedtest,
  dedupeProfiles,
  deleteProfiles,
  listProfiles,
  moveProfile,
  runSpeedtest,
  saveGroupProfile,
  saveProfile,
  setActiveProfile,
  sortProfiles,
  updateSubscriptions,
  useRuntimeEventStore,
} from "@/ipc";
import type {
  ProfileItem_Deserialize,
  ProfileListItem_Serialize,
  ProfileSortKey,
  SpeedActionType,
} from "@/ipc/bindings";
import { cn } from "@/lib/utils";

import { ImportProfilesDialog, SubscriptionsDialog } from "@/features/subscriptions";
import { CONFIG_TYPES, getProtocolLabel, MOVE_ACTIONS, SPEED_ACTIONS } from "./profile-constants";
import { ProfileDialog } from "./profile-dialog";
import { applyLiveUpdates } from "./server-table-live-updates";

type DialogState =
  | { mode: "create"; profile?: null }
  | { mode: "edit"; profile: ProfileListItem_Serialize }
  | null;

type ServerColumn = {
  cell: (item: ProfileListItem_Serialize, rowNumber: number) => React.ReactNode;
  id: string;
  label: string;
  sortKey?: ProfileSortKey;
  width: string;
};

const serverColumns: ServerColumn[] = [
  {
    cell: (item, rowNumber) => (
      <span className="flex items-center gap-2">
        {item.isActive ? (
          <span
            aria-label="Active profile"
            className="grid size-5 place-items-center rounded-full bg-primary text-primary-foreground"
            data-testid="active-profile-marker"
          >
            <Check className="size-3" aria-hidden="true" />
          </span>
        ) : (
          <span className="size-5 rounded-full border" aria-hidden="true" />
        )}
        <span className="tabular-nums text-muted-foreground">{rowNumber}</span>
      </span>
    ),
    id: "state",
    label: "#",
    width: "5rem",
  },
  {
    cell: (item) => getProtocolLabel(item.profile.ConfigType),
    id: "configType",
    label: "Protocol",
    sortKey: "configType",
    width: "8rem",
  },
  {
    cell: (item) => item.profile.Remarks || "Untitled",
    id: "remarks",
    label: "Remarks",
    sortKey: "remarks",
    width: "minmax(13rem,1.3fr)",
  },
  {
    cell: (item) => item.profile.Address,
    id: "address",
    label: "Address",
    sortKey: "address",
    width: "minmax(12rem,1fr)",
  },
  {
    cell: (item) => <span className="tabular-nums">{item.profile.Port || ""}</span>,
    id: "port",
    label: "Port",
    sortKey: "port",
    width: "5rem",
  },
  {
    cell: (item) => item.profile.Network || "tcp",
    id: "network",
    label: "Transport",
    sortKey: "network",
    width: "7rem",
  },
  {
    cell: (item) => item.profile.StreamSecurity || "none",
    id: "security",
    label: "Security",
    sortKey: "streamSecurity",
    width: "7rem",
  },
  {
    cell: (item) => formatDelay(item.profileEx.Delay),
    id: "delay",
    label: "Delay",
    sortKey: "delay",
    width: "6rem",
  },
  {
    cell: (item) => formatSpeedOrMessage(item.profileEx.Speed, item.profileEx.Message),
    id: "speed",
    label: "Speed",
    sortKey: "speed",
    width: "7rem",
  },
  {
    cell: (item) => formatTraffic(item.serverStat?.TodayUp),
    id: "todayUp",
    label: "Today up",
    width: "8rem",
  },
  {
    cell: (item) => formatTraffic(item.serverStat?.TodayDown),
    id: "todayDown",
    label: "Today down",
    width: "8rem",
  },
  {
    cell: (item) => formatTraffic(item.serverStat?.TotalUp),
    id: "totalUp",
    label: "Total up",
    width: "8rem",
  },
  {
    cell: (item) => formatTraffic(item.serverStat?.TotalDown),
    id: "totalDown",
    label: "Total down",
    width: "8rem",
  },
  {
    cell: (item) => item.profileEx.IpInfo ?? "",
    id: "ipInfo",
    label: "IP info",
    sortKey: "ipInfo",
    width: "10rem",
  },
  {
    cell: (item) => item.profile.Subid,
    id: "subid",
    label: "Group",
    sortKey: "subid",
    width: "8rem",
  },
];

const gridTemplateColumns = `2.75rem ${serverColumns.map((column) => column.width).join(" ")}`;

export function ProfilesScreen() {
  const [dialogState, setDialogState] = useState<DialogState>(null);
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [filterText, setFilterText] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [sortState, setSortState] = useState<{ ascending: boolean; key: ProfileSortKey } | null>(null);
  const [speedtestRunning, setSpeedtestRunning] = useState(false);
  const [subscriptionsOpen, setSubscriptionsOpen] = useState(false);
  const serverStatsByProfileId = useRuntimeEventStore((state) => state.serverStatsByProfileId);
  const speedtestResultsByProfileId = useRuntimeEventStore((state) => state.speedtestResultsByProfileId);
  const queryClient = useQueryClient();
  const filter = filterText.trim();
  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, filter || null),
    queryKey: profilesQueryKey(filter),
  });
  const profiles = useMemo(
    () => applyLiveUpdates(profilesQuery.data ?? [], serverStatsByProfileId, speedtestResultsByProfileId),
    [profilesQuery.data, serverStatsByProfileId, speedtestResultsByProfileId],
  );
  const tableColumns = useMemo<ColumnDef<ProfileListItem_Serialize>[]>(
    () =>
      serverColumns.map((column) => ({
        id: column.id,
        header: column.label,
      })),
    [],
  );
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Table owns stable row-model helpers internally.
  const table = useReactTable({
    columns: tableColumns,
    data: profiles,
    getCoreRowModel: getCoreRowModel(),
    getRowId: (row) => row.profile.IndexId,
  });
  const rows = table.getRowModel().rows;
  const viewportRef = useRef<HTMLDivElement>(null);
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    estimateSize: () => 38,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 520, width: 1200 },
    overscan: 10,
  });
  const visibleRows = rowVirtualizer.getVirtualItems();
  const renderedRows =
    visibleRows.length > 0
      ? visibleRows
      : rows.slice(0, Math.min(rows.length, 30)).map((row, index) => ({
          index,
          key: row.id,
          start: index * 38,
        }));
  const selected = profiles.filter((item) => selectedIds.has(item.profile.IndexId));
  const primarySelection = selected[0] ?? null;
  const allVisibleSelected = profiles.length > 0 && profiles.every((item) => selectedIds.has(item.profile.IndexId));

  async function runOperation(operation: () => Promise<unknown>) {
    setOperationError(null);
    try {
      await operation();
      await queryClient.invalidateQueries({ queryKey: ["profiles"] });
    } catch (error) {
      setOperationError(error instanceof Error ? error.message : String(error));
    }
  }

  function toggleSelection(indexId: string, selected: boolean) {
    setSelectedIds((current) => {
      const next = new Set(current);

      if (selected) {
        next.add(indexId);
      } else {
        next.delete(indexId);
      }

      return next;
    });
  }

  function selectOnly(indexId: string) {
    setSelectedIds(new Set([indexId]));
  }

  function toggleAllVisible(selected: boolean) {
    setSelectedIds(selected ? new Set(profiles.map((item) => item.profile.IndexId)) : new Set());
  }

  async function handleSort(sortKey: ProfileSortKey) {
    const ascending = sortState?.key === sortKey ? !sortState.ascending : true;
    setSortState({ ascending, key: sortKey });
    await runOperation(() => sortProfiles(null, sortKey, ascending));
  }

  async function handleSave(profile: ProfileItem_Deserialize) {
    const save = profile.ConfigType === CONFIG_TYPES.PolicyGroup || profile.ConfigType === CONFIG_TYPES.ProxyChain
      ? saveGroupProfile
      : saveProfile;
    await runOperation(() => save(profile));
    setDialogState(null);
  }

  const selectedIdsArray = selected.map((item) => item.profile.IndexId);

  async function handleSpeedtest(action: SpeedActionType, indexIds = selectedIdsArray) {
    setSpeedtestRunning(true);
    await runOperation(() => runSpeedtest(action, indexIds));
    setSpeedtestRunning(false);
  }

  async function handleCancelSpeedtest() {
    await runOperation(() => cancelSpeedtest());
    setSpeedtestRunning(false);
  }

  return (
    <section className="flex h-full min-h-0 flex-col" aria-label="Profiles">
      <div className="flex min-h-14 shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <Rows3 className="size-4 text-muted-foreground" aria-hidden="true" />
          <h2 className="text-sm font-semibold">Profiles</h2>
          <span className="rounded-md border px-2 py-1 text-xs text-muted-foreground">
            {profiles.length.toLocaleString()} rows
          </span>
        </div>

        <label className="ms-auto flex h-9 min-w-[14rem] items-center gap-2 rounded-md border bg-card px-3 text-sm">
          <Search className="size-4 text-muted-foreground" aria-hidden="true" />
          <span className="sr-only">Filter profiles</span>
          <input
            className="min-w-0 flex-1 bg-transparent outline-none placeholder:text-muted-foreground"
            onChange={(event) => setFilterText(event.target.value)}
            placeholder="Filter remarks or address"
            value={filterText}
          />
        </label>

        <Button onClick={() => setDialogState({ mode: "create" })} size="sm" type="button">
          <FilePlus2 className="size-4" aria-hidden="true" />
          Add
        </Button>
        <Button onClick={() => setImportOpen(true)} size="sm" type="button" variant="outline">
          <Upload className="size-4" aria-hidden="true" />
          Import
        </Button>
        <Button onClick={() => setSubscriptionsOpen(true)} size="sm" type="button" variant="outline">
          <Rss className="size-4" aria-hidden="true" />
          Subscriptions
        </Button>
        <Button onClick={() => void runOperation(() => updateSubscriptions(null, false, null))} size="sm" type="button" variant="outline">
          <RefreshCw className="size-4" aria-hidden="true" />
          Update subs
        </Button>
        <div className="flex items-center gap-1 border-s ps-2">
          <SpeedButton
            action={SPEED_ACTIONS.FastRealping}
            disabled={profiles.length === 0 || speedtestRunning}
            icon={Zap}
            label="Fast"
            onRun={handleSpeedtest}
          />
          <SpeedButton
            action={SPEED_ACTIONS.Tcping}
            disabled={selectedIdsArray.length === 0 || speedtestRunning}
            icon={Activity}
            label="TCP"
            onRun={handleSpeedtest}
          />
          <SpeedButton
            action={SPEED_ACTIONS.Realping}
            disabled={selectedIdsArray.length === 0 || speedtestRunning}
            icon={Clock}
            label="Real"
            onRun={handleSpeedtest}
          />
          <SpeedButton
            action={SPEED_ACTIONS.UdpTest}
            disabled={selectedIdsArray.length === 0 || speedtestRunning}
            icon={Radio}
            label="UDP"
            onRun={handleSpeedtest}
          />
          <SpeedButton
            action={SPEED_ACTIONS.Speedtest}
            disabled={selectedIdsArray.length === 0 || speedtestRunning}
            icon={Gauge}
            label="Speed"
            onRun={handleSpeedtest}
          />
          <SpeedButton
            action={SPEED_ACTIONS.Mixedtest}
            disabled={selectedIdsArray.length === 0 || speedtestRunning}
            icon={Wifi}
            label="Mixed"
            onRun={handleSpeedtest}
          />
          <Button
            disabled={!speedtestRunning}
            onClick={() => void handleCancelSpeedtest()}
            size="sm"
            title="Cancel speedtest"
            type="button"
            variant="outline"
          >
            <Square className="size-4" aria-hidden="true" />
            Stop
          </Button>
        </div>
        <Button
          disabled={!primarySelection}
          onClick={() => primarySelection && setDialogState({ mode: "edit", profile: primarySelection })}
          size="sm"
          type="button"
          variant="outline"
        >
          <Pencil className="size-4" aria-hidden="true" />
          Edit
        </Button>
        <Button
          disabled={!primarySelection}
          onClick={() => primarySelection && void runOperation(() => setActiveProfile(primarySelection.profile.IndexId))}
          size="sm"
          type="button"
          variant="outline"
        >
          <Play className="size-4" aria-hidden="true" />
          Activate
        </Button>
        <Button
          disabled={selectedIdsArray.length === 0}
          onClick={() => void runOperation(() => copyProfiles(selectedIdsArray))}
          size="sm"
          type="button"
          variant="outline"
        >
          <Copy className="size-4" aria-hidden="true" />
          Copy
        </Button>
        <Button
          disabled={selectedIdsArray.length === 0}
          onClick={() => void runOperation(() => deleteProfiles(selectedIdsArray))}
          size="sm"
          type="button"
          variant="outline"
        >
          <Trash2 className="size-4" aria-hidden="true" />
          Delete
        </Button>
        <Button onClick={() => void runOperation(() => dedupeProfiles(null, null))} size="sm" type="button" variant="outline">
          <Filter className="size-4" aria-hidden="true" />
          Dedupe
        </Button>
      </div>

      {operationError ? (
        <div className="border-b bg-destructive/10 px-4 py-2 text-sm text-destructive" role="alert">
          {operationError}
        </div>
      ) : null}

      <div className="min-h-0 flex-1 overflow-hidden p-4">
        <div
          aria-busy={profilesQuery.isLoading}
          aria-colcount={serverColumns.length + 1}
          aria-label="Profiles"
          aria-rowcount={profiles.length}
          className="flex h-full min-h-[18rem] flex-col overflow-hidden rounded-md border bg-card"
          role="table"
        >
          <div className="overflow-x-auto border-b">
            <div
              aria-rowindex={1}
              className="grid min-w-[110rem] items-center bg-muted text-xs font-semibold uppercase text-muted-foreground"
              role="row"
              style={{ gridTemplateColumns }}
            >
              <div
                aria-colindex={1}
                className="flex h-9 items-center justify-center border-e px-2"
                role="columnheader"
              >
                <input
                  aria-label="Select all profiles"
                  checked={allVisibleSelected}
                  className="size-4 accent-primary"
                  onChange={(event) => toggleAllVisible(event.target.checked)}
                  type="checkbox"
                />
              </div>
              {serverColumns.map((column, columnIndex) => (
                <div
                  aria-colindex={columnIndex + 2}
                  aria-sort={sortAriaValue(column, sortState)}
                  className="flex h-9 min-w-0 items-center border-e px-2 last:border-e-0"
                  key={column.id}
                  role="columnheader"
                >
                  {column.sortKey ? (
                    <button
                      className="flex min-w-0 items-center gap-1 text-start"
                      onClick={() => void handleSort(column.sortKey!)}
                      type="button"
                    >
                      <span className="truncate">{column.label}</span>
                      {sortState?.key === column.sortKey ? (
                        sortState.ascending ? (
                          <ArrowUp className="size-3" aria-hidden="true" />
                        ) : (
                          <ArrowDown className="size-3" aria-hidden="true" />
                        )
                      ) : null}
                    </button>
                  ) : (
                    <span className="truncate">{column.label}</span>
                  )}
                </div>
              ))}
            </div>
          </div>

          <div
            className="min-h-0 flex-1 overflow-auto"
            data-testid="server-table-viewport"
            ref={viewportRef}
            role="rowgroup"
          >
            {profilesQuery.isLoading ? (
              <div className="grid h-full place-items-center text-sm text-muted-foreground" role="status">
                Loading profiles
              </div>
            ) : rows.length === 0 ? (
              <div className="grid h-full place-items-center text-sm text-muted-foreground" role="status">
                No profiles
              </div>
            ) : (
              <div className="relative min-w-[110rem]" style={{ height: rowVirtualizer.getTotalSize() }}>
                {renderedRows.map((virtualRow) => {
                  const row = rows[virtualRow.index];
                  const item = row.original;
                  const indexId = item.profile.IndexId;
                  const isSelected = selectedIds.has(indexId);

                  return (
                    <ProfileRowContextMenu
                      item={item}
                      key={row.id}
                      onActivate={() => void runOperation(() => setActiveProfile(indexId))}
                      onCopy={() => void runOperation(() => copyProfiles(selectedIds.has(indexId) ? selectedIdsArray : [indexId]))}
                      onDelete={() => void runOperation(() => deleteProfiles(selectedIds.has(indexId) ? selectedIdsArray : [indexId]))}
                      onEdit={() => setDialogState({ mode: "edit", profile: item })}
                      onMove={(action) => void runOperation(() => moveProfile(null, indexId, action, null))}
                      onSelectOnly={() => selectOnly(indexId)}
                    >
                      <div
                        aria-selected={isSelected}
                        className={cn(
                          "absolute start-0 grid h-[38px] min-w-[110rem] items-center border-b text-sm outline-none",
                          item.isActive ? "bg-accent/70" : virtualRow.index % 2 === 0 ? "bg-card" : "bg-background",
                          isSelected ? "ring-1 ring-inset ring-primary" : null,
                        )}
                        data-testid="server-row"
                        draggable
                        onClick={(event) => {
                          if (event.metaKey || event.ctrlKey) {
                            toggleSelection(indexId, !isSelected);
                          } else {
                            selectOnly(indexId);
                          }
                        }}
                        onDoubleClick={() => void runOperation(() => setActiveProfile(indexId))}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            event.preventDefault();
                            void runOperation(() => setActiveProfile(indexId));
                          }
                          if (event.key === " ") {
                            event.preventDefault();
                            toggleSelection(indexId, !isSelected);
                          }
                        }}
                        onDragOver={(event) => event.preventDefault()}
                        onDragStart={(event) => {
                          setDraggedId(indexId);
                          event.dataTransfer.effectAllowed = "move";
                          event.dataTransfer.setData("text/profile-id", indexId);
                        }}
                        onDrop={(event) => {
                          event.preventDefault();
                          const sourceId = event.dataTransfer.getData("text/profile-id") || draggedId;
                          if (sourceId && sourceId !== indexId) {
                            void runOperation(() => moveProfile(null, sourceId, MOVE_ACTIONS.Position, virtualRow.index));
                          }
                          setDraggedId(null);
                        }}
                        aria-rowindex={virtualRow.index + 2}
                        role="row"
                        style={{
                          gridTemplateColumns,
                          transform: `translateY(${virtualRow.start}px)`,
                        }}
                        tabIndex={0}
                      >
                        <div
                          aria-colindex={1}
                          className="flex h-full items-center justify-center border-e px-2"
                          role="cell"
                        >
                          <input
                            aria-label={`Select ${item.profile.Remarks || indexId}`}
                            checked={isSelected}
                            className="size-4 accent-primary"
                            onChange={(event) => toggleSelection(indexId, event.target.checked)}
                            onClick={(event) => event.stopPropagation()}
                            type="checkbox"
                          />
                        </div>
                        {serverColumns.map((column, columnIndex) => {
                          const cell = column.cell(item, virtualRow.index + 1);

                          return (
                            <div
                              aria-colindex={columnIndex + 2}
                              className="flex h-full min-w-0 items-center border-e px-2 last:border-e-0"
                              key={column.id}
                              role="cell"
                              title={cellTitle(cell)}
                            >
                              <span className="truncate">{cell}</span>
                            </div>
                          );
                        })}
                      </div>
                    </ProfileRowContextMenu>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </div>

      <ProfileDialog
        mode={dialogState?.mode ?? "create"}
        onOpenChange={(open) => !open && setDialogState(null)}
        onSubmit={handleSave}
        open={Boolean(dialogState)}
        profile={dialogState?.mode === "edit" ? dialogState.profile : null}
      />
      <ImportProfilesDialog
        onImported={() => void queryClient.invalidateQueries({ queryKey: ["profiles"] })}
        onOpenChange={setImportOpen}
        open={importOpen}
      />
      <SubscriptionsDialog
        onChanged={() => void queryClient.invalidateQueries({ queryKey: ["profiles"] })}
        onOpenChange={setSubscriptionsOpen}
        open={subscriptionsOpen}
      />
    </section>
  );
}

function SpeedButton({
  action,
  disabled,
  icon: Icon,
  label,
  onRun,
}: {
  action: SpeedActionType;
  disabled: boolean;
  icon: LucideIcon;
  label: string;
  onRun: (action: SpeedActionType) => Promise<void>;
}) {
  return (
    <Button
      disabled={disabled}
      onClick={() => void onRun(action)}
      size="sm"
      title={`${label} speedtest`}
      type="button"
      variant="outline"
    >
      <Icon className="size-4" aria-hidden="true" />
      {label}
    </Button>
  );
}

function ProfileRowContextMenu({
  children,
  item,
  onActivate,
  onCopy,
  onDelete,
  onEdit,
  onMove,
  onSelectOnly,
}: {
  children: React.ReactNode;
  item: ProfileListItem_Serialize;
  onActivate: () => void;
  onCopy: () => void;
  onDelete: () => void;
  onEdit: () => void;
  onMove: (action: number) => void;
  onSelectOnly: () => void;
}) {
  return (
    <ContextMenu.Root onOpenChange={(open) => open && onSelectOnly()}>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="z-50 min-w-48 rounded-md border bg-card p-1 text-sm shadow-xl outline-none">
          <ContextMenu.Label className="truncate px-2 py-1.5 text-xs text-muted-foreground">
            {item.profile.Remarks || "Untitled"}
          </ContextMenu.Label>
          <ContextItem icon={Play} label="Activate" onSelect={onActivate} />
          <ContextItem icon={Pencil} label="Edit" onSelect={onEdit} />
          <ContextItem icon={Copy} label="Copy" onSelect={onCopy} />
          <ContextItem icon={Trash2} label="Delete" onSelect={onDelete} />
          <ContextMenu.Separator className="my-1 h-px bg-border" />
          <ContextItem icon={ChevronsUp} label="Move to top" onSelect={() => onMove(MOVE_ACTIONS.Top)} />
          <ContextItem icon={ArrowUp} label="Move up" onSelect={() => onMove(MOVE_ACTIONS.Up)} />
          <ContextItem icon={ArrowDown} label="Move down" onSelect={() => onMove(MOVE_ACTIONS.Down)} />
          <ContextItem icon={ChevronsDown} label="Move to bottom" onSelect={() => onMove(MOVE_ACTIONS.Bottom)} />
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

function ContextItem({
  icon: Icon,
  label,
  onSelect,
}: {
  icon: LucideIcon;
  label: string;
  onSelect: () => void;
}) {
  return (
    <ContextMenu.Item
      className="flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 outline-none focus:bg-accent focus:text-accent-foreground"
      onSelect={onSelect}
    >
      <Icon className="size-4" aria-hidden="true" />
      {label}
    </ContextMenu.Item>
  );
}

function profilesQueryKey(filter: string) {
  return ["profiles", { filter }] as const;
}

function sortAriaValue(
  column: ServerColumn,
  sortState: { ascending: boolean; key: ProfileSortKey } | null,
) {
  if (!column.sortKey || sortState?.key !== column.sortKey) {
    return "none" as const;
  }

  return sortState.ascending ? "ascending" : "descending";
}

function cellTitle(cell: React.ReactNode) {
  return typeof cell === "string" || typeof cell === "number" ? String(cell) : undefined;
}

function formatDelay(delay: number) {
  return delay > 0 ? `${delay} ms` : "";
}

function formatSpeed(speed: number | null) {
  if (!speed || speed <= 0) {
    return "";
  }

  if (speed >= 1024 * 1024) {
    return `${(speed / 1024 / 1024).toFixed(1)} MB/s`;
  }
  if (speed >= 1024) {
    return `${(speed / 1024).toFixed(1)} KB/s`;
  }

  return `${speed.toFixed(0)} B/s`;
}

function formatSpeedOrMessage(speed: number | null, message?: string | null) {
  const speedLabel = formatSpeed(speed);

  if (speedLabel) {
    return speedLabel;
  }

  if (!message || /^-?\d+(\.\d+)?$/.test(message)) {
    return "";
  }

  return message;
}

function formatTraffic(value: number | null | undefined) {
  if (!value || value <= 0) {
    return "";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let scaled = value;
  let unitIndex = 0;
  while (scaled >= 1024 && unitIndex < units.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }

  return `${scaled >= 10 ? scaled.toFixed(0) : scaled.toFixed(1)} ${units[unitIndex]}`;
}
