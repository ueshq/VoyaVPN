import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDownToLine, ScrollText, Search, Trash2 } from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { Input } from "@/components/ui/input";
import { useI18n } from "@/i18n/use-i18n";
import { useRuntimeEventStore } from "@/ipc";
import type { LogLevel } from "@/ipc/bindings";
import { cn } from "@/lib/utils";

// Severity order; drives both the filter chips and their visual weight.
const LOG_LEVELS: LogLevel[] = ["trace", "debug", "info", "warn", "error"];
// Fixed row height keeps the virtualizer geometry exact (mirrors the profiles
// and connections tables) and lets auto-scroll-to-bottom land precisely.
const ROW_HEIGHT = 28;
// Treat the viewport as "parked at the bottom" within this slack so wheel
// momentum or sub-pixel rounding does not break the follow-tail behaviour.
const STICK_THRESHOLD = 24;

export function LogsScreen() {
  const { t } = useI18n();
  const clearLogs = useRuntimeEventStore((state) => state.clearLogs);
  const logLines = useRuntimeEventStore((state) => state.logLines);

  const [search, setSearch] = useState("");
  const [activeLevels, setActiveLevels] = useState<Set<LogLevel>>(() => new Set(LOG_LEVELS));

  // LogLineEvent carries no timestamp, so stamp each id the first render it
  // appears and prune ids the store has dropped (it keeps only the last 500).
  const timestampsRef = useRef<Map<number, string>>(new Map());
  const timestamps = timestampsRef.current;
  const liveIds = new Set<number>();
  for (const line of logLines) {
    liveIds.add(line.id);
    if (!timestamps.has(line.id)) {
      timestamps.set(line.id, formatTimestamp(new Date()));
    }
  }
  if (timestamps.size > liveIds.size) {
    for (const id of timestamps.keys()) {
      if (!liveIds.has(id)) {
        timestamps.delete(id);
      }
    }
  }

  const needle = search.trim().toLowerCase();
  const filtered = useMemo(
    () =>
      logLines.filter(
        (line) => activeLevels.has(line.level) && (needle === "" || line.line.toLowerCase().includes(needle)),
      ),
    [activeLevels, logLines, needle],
  );

  const viewportRef = useRef<HTMLDivElement>(null);
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual exposes scroll helpers that React Compiler cannot memoize safely.
  const rowVirtualizer = useVirtualizer({
    count: filtered.length,
    estimateSize: () => ROW_HEIGHT,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 480, width: 800 },
    overscan: 16,
  });

  // Follow the newest line while the viewport sits at the bottom; release the
  // moment the user scrolls up, and resume when they return to the bottom.
  const [atBottom, setAtBottom] = useState(true);
  const atBottomRef = useRef(true);

  function handleScroll() {
    const element = viewportRef.current;
    if (!element) {
      return;
    }

    const next = element.scrollHeight - element.scrollTop - element.clientHeight <= STICK_THRESHOLD;
    atBottomRef.current = next;
    setAtBottom(next);
  }

  useEffect(() => {
    if (atBottomRef.current && filtered.length > 0) {
      rowVirtualizer.scrollToIndex(filtered.length - 1, { align: "end" });
    }
  }, [filtered.length, rowVirtualizer]);

  function scrollToLatest() {
    atBottomRef.current = true;
    setAtBottom(true);
    if (filtered.length > 0) {
      rowVirtualizer.scrollToIndex(filtered.length - 1, { align: "end" });
    }
  }

  function toggleLevel(level: LogLevel) {
    setActiveLevels((current) => {
      const next = new Set(current);
      if (next.has(level)) {
        next.delete(level);
      } else {
        next.add(level);
      }

      return next;
    });
  }

  const hasLogs = logLines.length > 0;
  const isFiltered = needle !== "" || activeLevels.size !== LOG_LEVELS.length;
  const virtualRows = rowVirtualizer.getVirtualItems();
  // jsdom (and the very first paint) yields no virtual items because the scroll
  // element has no measured height; fall back to a head slice so rows render.
  const renderedRows =
    virtualRows.length > 0
      ? virtualRows
      : filtered.slice(0, Math.min(filtered.length, 50)).map((_, index) => ({
          index,
          start: index * ROW_HEIGHT,
        }));

  return (
    <section className="flex h-full min-h-0 flex-col" aria-label={t("tabs.logs")}>
      <div className="flex min-h-14 shrink-0 flex-wrap items-center gap-2 border-b px-4 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <ScrollText className="size-4 text-muted-foreground" aria-hidden="true" />
          <h2 className="text-sm font-semibold">{t("tabs.logs")}</h2>
          {hasLogs ? (
            <Badge className="h-6 bg-background font-mono tabular-nums text-muted-foreground" variant="outline">
              {isFiltered
                ? `${filtered.length.toLocaleString()} / ${logLines.length.toLocaleString()}`
                : logLines.length.toLocaleString()}
            </Badge>
          ) : null}
        </div>

        <div className="relative ms-auto min-w-[12rem] flex-1 sm:flex-none">
          <Search
            className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            aria-label={t("panes.logs.search")}
            className="h-9 ps-9 font-mono"
            onChange={(event) => setSearch(event.target.value)}
            placeholder={t("panes.logs.search")}
            type="search"
            value={search}
          />
        </div>

        <div className="flex items-center gap-1" role="group" aria-label={t("panes.logs.levelFilterLabel")}>
          {LOG_LEVELS.map((level) => {
            const active = activeLevels.has(level);

            return (
              <Button
                aria-label={t("panes.logs.toggleLevel", { level })}
                aria-pressed={active}
                className={cn(
                  "h-7 px-2 font-mono text-[11px] uppercase",
                  active ? logLevelClassName(level) : "text-muted-foreground line-through opacity-60",
                )}
                key={level}
                onClick={() => toggleLevel(level)}
                size="sm"
                type="button"
                variant="outline"
              >
                {level}
              </Button>
            );
          })}
        </div>

        <Button className="gap-2" onClick={clearLogs} size="sm" type="button" variant="outline">
          <Trash2 className="size-4" aria-hidden="true" />
          {t("actions.clear")}
        </Button>
      </div>

      {!hasLogs ? (
        <EmptyState className="min-h-0 flex-1 content-center" icon={ScrollText} title={t("panes.logs.empty")} />
      ) : (
        <div className="relative min-h-0 flex-1">
          <div className="h-full overflow-auto bg-muted/20" onScroll={handleScroll} ref={viewportRef}>
            {filtered.length === 0 ? (
              <EmptyState
                className="h-full content-center"
                description={t("panes.logs.noMatchesDescription")}
                icon={Search}
                title={t("panes.logs.noMatches")}
              />
            ) : (
              <ol
                className="relative w-full p-2 font-mono text-xs leading-5"
                data-testid="log-lines"
                style={{ height: rowVirtualizer.getTotalSize() + 16 }}
              >
                {renderedRows.map((virtualRow) => {
                  const line = filtered[virtualRow.index];
                  if (!line) {
                    return null;
                  }

                  return (
                    <li
                      className="absolute inset-x-2 grid grid-cols-[4.25rem_3.5rem_minmax(0,1fr)] items-center gap-3 rounded-md px-2 transition-colors hover:bg-background/80"
                      data-testid="log-line"
                      key={line.id}
                      style={{ height: ROW_HEIGHT, transform: `translateY(${virtualRow.start}px)` }}
                    >
                      <time className="tabular-nums text-muted-foreground">{timestamps.get(line.id) ?? ""}</time>
                      <Badge
                        className={cn(
                          "h-5 justify-center rounded-sm px-1.5 font-mono uppercase",
                          logLevelClassName(line.level),
                        )}
                        variant="outline"
                      >
                        {line.level}
                      </Badge>
                      <span className="truncate text-foreground" title={line.line}>
                        {line.line}
                      </span>
                    </li>
                  );
                })}
              </ol>
            )}
          </div>

          {!atBottom && filtered.length > 0 ? (
            <Button
              className="absolute bottom-4 end-4 gap-2 shadow-md"
              onClick={scrollToLatest}
              size="sm"
              type="button"
              variant="secondary"
            >
              <ArrowDownToLine className="size-4" aria-hidden="true" />
              {t("panes.logs.jumpToLatest")}
            </Button>
          ) : null}
        </div>
      )}
    </section>
  );
}

function formatTimestamp(date: Date) {
  const pad = (value: number) => value.toString().padStart(2, "0");

  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

function logLevelClassName(level: LogLevel) {
  switch (level) {
    case "trace":
    case "debug":
      return "border-transparent bg-muted text-muted-foreground";
    case "info":
      return "bg-background text-foreground";
    case "warn":
      return "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300";
    case "error":
      return "border-destructive/30 bg-destructive/10 text-destructive";
  }
}
