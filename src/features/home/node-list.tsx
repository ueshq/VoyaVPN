import { useMemo, useState } from "react";
import { LoaderCircle, Search } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { dataTableRowHover, dataTableRowSelected } from "@/components/app-shell/data-table-surface";
import { useI18n } from "@/i18n/use-i18n";
import type { ProfileListItem_Serialize } from "@/ipc/bindings";
import { formatDelay } from "@/lib/formatting";
import { cn } from "@/lib/utils";

import { getProtocolLabel } from "@/features/profiles/profile-constants";

/**
 * Always-visible node list for the Home screen. A controlled, IPC-free component:
 * single-click (or Space) selects a row locally (blue highlight via
 * {@link dataTableRowSelected}); double-click (or Enter) activates it — the parent
 * decides what "activate" does (switch + connect/restart). The green dot
 * (`bg-connected`) marks the node that is actually running (`runningId`), kept
 * distinct from the blue local selection. Rows reuse the former node-picker row
 * markup and the server-table formatting helpers; no new IPC is introduced.
 */
export function NodeList({
  isPending,
  onActivate,
  onSelect,
  profiles,
  runningId,
  selectedId,
  switchingId,
}: {
  isPending: boolean;
  onActivate: (indexId: string) => void;
  onSelect: (indexId: string) => void;
  profiles: ProfileListItem_Serialize[];
  runningId: string | null;
  selectedId: string | null;
  switchingId: string | null;
}) {
  const { t } = useI18n();
  const [filterText, setFilterText] = useState("");

  // Keep the imported order stable (no active-pin sort) so rows never jump
  // around in an always-visible list; only filter by remarks / address.
  const filtered = useMemo<ProfileListItem_Serialize[]>(() => {
    const query = filterText.trim().toLowerCase();
    if (!query) {
      return profiles;
    }

    return profiles.filter(
      (item) =>
        item.profile.Remarks.toLowerCase().includes(query) ||
        item.profile.Address.toLowerCase().includes(query),
    );
  }, [profiles, filterText]);

  const showEmpty = !isPending && filtered.length === 0;

  return (
    <div className="flex min-h-0 w-full flex-1 flex-col gap-3">
      <div className="relative shrink-0">
        <Search
          aria-hidden="true"
          className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          aria-label={t("home.searchNodes")}
          className="ps-9"
          onChange={(event) => setFilterText(event.target.value)}
          placeholder={t("home.searchNodes")}
          value={filterText}
        />
      </div>

      <ScrollArea className="-mx-2 min-h-32 flex-1 px-2">
        <ul aria-label={t("home.selectNode")} className="flex flex-col gap-0.5" role="listbox">
          {filtered.map((item) => {
            const indexId = item.profile.IndexId;
            const selected = selectedId === indexId;
            const running = runningId === indexId;
            const switching = switchingId === indexId;
            const delay = formatDelay(item.profileEx.Delay);

            return (
              <li key={indexId} role="presentation">
                <div
                  aria-busy={switching || undefined}
                  aria-selected={selected}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-start transition-colors",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    selected ? dataTableRowSelected : dataTableRowHover,
                    switching && "pointer-events-none opacity-60",
                  )}
                  onClick={() => onSelect(indexId)}
                  onDoubleClick={() => onActivate(indexId)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      onActivate(indexId);
                    }
                    if (event.key === " ") {
                      event.preventDefault();
                      onSelect(indexId);
                    }
                  }}
                  role="option"
                  tabIndex={0}
                >
                  <span aria-hidden="true" className="flex size-2 shrink-0 items-center justify-center">
                    {running ? <span className="size-1.5 rounded-full bg-connected" /> : null}
                  </span>

                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="flex items-center gap-2">
                      <span className="truncate text-sm font-medium">
                        {item.profile.Remarks || t("panes.profiles.untitled")}
                      </span>
                      <Badge className="shrink-0" variant="outline">
                        {getProtocolLabel(item.profile.ConfigType)}
                      </Badge>
                    </span>
                    <span className="truncate text-xs text-muted-foreground">
                      {item.profile.Address}:{item.profile.Port}
                    </span>
                  </span>

                  {switching ? (
                    <LoaderCircle aria-hidden="true" className="size-4 shrink-0 animate-spin text-muted-foreground" />
                  ) : delay ? (
                    <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{delay}</span>
                  ) : null}
                </div>
              </li>
            );
          })}

          {showEmpty ? (
            <li className="px-3 py-6 text-center text-sm text-muted-foreground" role="presentation">
              {t("home.noNodes")}
            </li>
          ) : null}
        </ul>
      </ScrollArea>
    </div>
  );
}
