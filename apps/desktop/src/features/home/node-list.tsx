import { useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { LoaderCircle, Search } from "lucide-react";

import { Badge } from "@voya/ui/components/badge";
import { Input } from "@voya/ui/components/input";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import { dataTableRowHover, dataTableRowSelected } from "@/components/app-shell/data-table-surface";
import { useI18n } from "@voya/i18n/use-i18n";
import type { ProfileListEntry } from "@/ipc/bindings";
import { formatDelay } from "@voya/utils/formatting";
import { cn } from "@voya/ui/lib/utils";

import { getProtocolLabel } from "@/features/profiles/profile-constants";
import { profileAddress, profilePort } from "@/features/profiles/profile-display";

/**
 * Always-visible node list for the Home screen. A controlled, IPC-free component:
 * single-click (or Space) selects a row locally (blue highlight via
 * {@link dataTableRowSelected}); double-click (or Enter) activates it — the parent
 * decides what "activate" does (switch + connect/restart). While a runtime
 * action is in flight (`busy`) activation is refused, exactly like the primary
 * connect button, so a switch can never race a pending connect/disconnect;
 * local selection stays available. The green dot (`bg-connected`) marks the node
 * that is actually running (`runningId`), kept distinct from the blue local
 * selection. Rows reuse the former node-picker row markup and the server-table
 * formatting helpers; no new IPC is introduced.
 *
 * Keyboard behaviour follows the listbox pattern the roles promise: one tab stop
 * for the whole list (roving tabindex) with Up/Down/Home/End moving the active
 * option, so a large subscription does not put hundreds of tab stops between the
 * search box and the controls below it.
 */
export function NodeList({
  busy,
  isPending,
  onActivate,
  onSelect,
  profiles,
  runningId,
  selectedId,
  switchingId,
}: {
  busy: boolean;
  isPending: boolean;
  onActivate: (indexId: string) => void;
  onSelect: (indexId: string) => void;
  profiles: ProfileListEntry[];
  runningId: string | null;
  selectedId: string | null;
  switchingId: string | null;
}) {
  const { t } = useI18n();
  const [filterText, setFilterText] = useState("");
  const listRef = useRef<HTMLUListElement>(null);

  // Keep the imported order stable (no active-pin sort) so rows never jump
  // around in an always-visible list; only filter by remarks / address.
  const filtered = useMemo<ProfileListEntry[]>(() => {
    const query = filterText.trim().toLowerCase();
    if (!query) {
      return profiles;
    }

    return profiles.filter(
      (item) =>
        item.profile.remarks.toLowerCase().includes(query) ||
        profileAddress(item.profile).toLowerCase().includes(query),
    );
  }, [profiles, filterText]);

  const showEmpty = !isPending && filtered.length === 0;
  const selectedIndex = filtered.findIndex((item) => item.profile.id === selectedId);
  // With nothing selected the first row carries the tab stop, so the list is
  // always reachable from the keyboard.
  const tabStopIndex = selectedIndex >= 0 ? selectedIndex : 0;

  function moveActiveOption(event: KeyboardEvent<HTMLUListElement>) {
    // Move relative to the option that actually holds focus, which is the tab
    // stop. Anchoring on `selectedIndex` would make the first Arrow press with
    // nothing selected re-select the row the user is already standing on.
    const nextIndex = nextOptionIndex(event.key, tabStopIndex, filtered.length);
    const next = nextIndex === null ? undefined : filtered[nextIndex];
    if (nextIndex === null || !next) {
      return;
    }

    event.preventDefault();
    onSelect(next.profile.id);
    listRef.current?.querySelectorAll<HTMLElement>("[role=\"option\"]")[nextIndex]?.focus();
  }

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
        <ul
          aria-label={t("home.selectNode")}
          className="flex flex-col gap-0.5"
          onKeyDown={moveActiveOption}
          ref={listRef}
          role="listbox"
        >
          {filtered.map((item, index) => {
            const indexId = item.profile.id;
            const selected = selectedId === indexId;
            const running = runningId === indexId;
            const switching = switchingId === indexId;
            const delay = formatDelay(item.metrics.delayMs);

            return (
              <li key={indexId} role="presentation">
                <div
                  aria-busy={switching || undefined}
                  aria-disabled={busy || undefined}
                  aria-selected={selected}
                  className={cn(
                    "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-start transition-colors",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    selected ? dataTableRowSelected : dataTableRowHover,
                    switching && "pointer-events-none opacity-60",
                  )}
                  onClick={() => onSelect(indexId)}
                  onDoubleClick={() => {
                    if (busy) {
                      return;
                    }
                    onActivate(indexId);
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      if (!busy) {
                        onActivate(indexId);
                      }
                    }
                    if (event.key === " ") {
                      event.preventDefault();
                      onSelect(indexId);
                    }
                  }}
                  role="option"
                  tabIndex={index === tabStopIndex ? 0 : -1}
                >
                  <span aria-hidden="true" className="flex size-2 shrink-0 items-center justify-center">
                    {running ? <span className="size-1.5 rounded-full bg-connected" /> : null}
                  </span>

                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="flex items-center gap-2">
                      <span className="truncate text-sm font-medium">
                        {item.profile.remarks || t("panes.profiles.untitled")}
                      </span>
                      <Badge className="shrink-0" variant="outline">
                        {getProtocolLabel(item.profile.protocol.kind)}
                      </Badge>
                    </span>
                    <span className="truncate text-xs text-muted-foreground">
                      {profileAddress(item.profile)}:{profilePort(item.profile)}
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

/** Listbox arrow-key semantics: Down/Up step and clamp, Home/End jump. */
function nextOptionIndex(key: string, currentIndex: number, count: number): number | null {
  if (count === 0) {
    return null;
  }

  switch (key) {
    case "ArrowDown":
      return currentIndex < 0 ? 0 : Math.min(currentIndex + 1, count - 1);
    case "ArrowUp":
      return currentIndex < 0 ? count - 1 : Math.max(currentIndex - 1, 0);
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}
