import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDownToLine,
  ClipboardCopy,
  FileDown,
  ScrollText,
  Search,
  Trash2,
} from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { EmptyState } from "@voya/ui/components/empty-state";
import { SearchInput } from "@voya/ui/components/search-input";
import { MenubarItem } from "@voya/ui/components/menubar";
import { MoreMenu } from "@voya/ui/components/row-menus";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@voya/ui/components/select";
import type { TranslationFunction } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";

import { formatTimeOfDay } from "@voya/utils/formatting";
import { exportLogs } from "@/ipc/commands";
import { logLineText } from "@voya/client/messages";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { StoredLogLine } from "@voya/client/runtime-event-store";
import type { LogLevel } from "@/ipc/bindings";
import { restoreFocus } from "@voya/ui/lib/focus";
import { cn } from "@voya/ui/lib/utils";
import { PageHeader } from "@/components/app-shell/page-section";
import { writeClipboard } from "@/lib/clipboard";
import { firstPaintVirtualItems } from "@/lib/virtual-list";
import { toastError, useToastStore } from "@voya/client/toast-store";

import { useLogStream } from "@voya/features/logs/use-log-stream";

export type LogFilter = "standard" | "issues" | "all";
const ROW_HEIGHT = 36;
const STICK_THRESHOLD = 24;

type ResolvedLogLine = StoredLogLine & { searchText: string; text: string };

/**
 * Translated lines, keyed by the stored line. The store keeps each line's
 * object from frame to frame, so a line is translated once per language
 * instead of all 500 on every flush; entries leave with their lines.
 */
const resolvedLines = new WeakMap<
  StoredLogLine,
  { resolved: ResolvedLogLine; t: TranslationFunction }
>();

function resolveLogLine(t: TranslationFunction, line: StoredLogLine): ResolvedLogLine {
  const cached = resolvedLines.get(line);
  if (cached?.t === t) return cached.resolved;
  const text = logLineText(t, line.body);
  const resolved = { ...line, searchText: text.toLowerCase(), text };
  resolvedLines.set(line, { resolved, t });
  return resolved;
}

export function LogsPanel({
  search,
  onSearchChange,
  filter,
  onFilterChange,
}: {
  search: string;
  onSearchChange: (value: string) => void;
  filter: LogFilter;
  onFilterChange: (value: LogFilter) => void;
}) {
  const { t } = useI18n();
  useLogStream();
  const clearLogs = useRuntimeEventStore((state) => state.clearLogs);
  const logLines = useRuntimeEventStore((state) => state.logLines);
  const [selected, setSelected] = useState<StoredLogLine | null>(null);
  const returnFocusRef = useRef<HTMLButtonElement | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const needle = search.trim().toLowerCase();
  const resolved = useMemo(
    () => logLines.map((line) => resolveLogLine(t, line)),
    [logLines, t],
  );
  const filtered = useMemo(
    () =>
      resolved.filter((line) => {
        const included =
          filter === "all" ||
          line.level === "warn" ||
          line.level === "error" ||
          (filter === "standard" && line.level === "info");
        return (
          included && (!needle || line.searchText.includes(needle))
        );
      }),
    [filter, needle, resolved],
  );
  const pushToast = useToastStore((state) => state.pushToast);
  // Copy and export take what the filter and search currently show.
  const shownText = () =>
    filtered
      .map(
        (line) =>
          `${formatTimeOfDay(line.loggedAt)} [${line.level}] ${line.text}`,
      )
      .join("\n");

  async function copyShown() {
    try {
      await writeClipboard(shownText());
      pushToast({
        description: t("panes.logs.copied", { count: filtered.length }),
        severity: "info",
        title: t("panes.logs.copy"),
      });
    } catch (error) {
      toastError(t("panes.logs.copyFailed"), error);
    }
  }

  async function exportShown() {
    try {
      if (await exportLogs(shownText())) {
        pushToast({
          description: t("panes.logs.exported"),
          severity: "info",
          title: t("panes.logs.export"),
        });
      }
    } catch (error) {
      toastError(t("panes.logs.exportFailed"), error);
    }
  }

  const levels: Record<LogLevel, string> = {
    trace: t("panes.logs.levels.trace"),
    debug: t("panes.logs.levels.debug"),
    info: t("panes.logs.levels.info"),
    warn: t("panes.logs.levels.warn"),
    error: t("panes.logs.levels.error"),
  };
  const viewportRef = useRef<HTMLDivElement>(null);
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual exposes scroll helpers that React Compiler cannot memoize safely.
  const virtualizer = useVirtualizer({
    count: filtered.length,
    estimateSize: () => ROW_HEIGHT,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 480, width: 800 },
    overscan: 16,
  });
  const [atBottom, setAtBottom] = useState(true);
  const atBottomRef = useRef(true);
  function handleScroll() {
    const element = viewportRef.current;
    if (!element) return;
    const next =
      element.scrollHeight - element.scrollTop - element.clientHeight <=
      STICK_THRESHOLD;
    atBottomRef.current = next;
    setAtBottom(next);
  }
  // The buffer stays at 500 when it rolls over, so length alone cannot tell us
  // that a new line arrived. Follow the last ID as well as the number of rows.
  const latestId = filtered.at(-1)?.id;
  useEffect(() => {
    if (atBottomRef.current && filtered.length > 0) {
      virtualizer.scrollToIndex(filtered.length - 1, { align: "end" });
    }
  }, [filtered.length, latestId, virtualizer]);

  function scrollToLatest() {
    atBottomRef.current = true;
    setAtBottom(true);
    if (filtered.length > 0)
      virtualizer.scrollToIndex(filtered.length - 1, { align: "end" });
  }
  const renderedRows = firstPaintVirtualItems(
    virtualizer.getVirtualItems(),
    filtered.length,
    ROW_HEIGHT,
    50,
  );

  return (
    <section
      aria-label={t("tabs.logs")}
      className="flex h-full min-h-0 min-w-0 flex-col"
    >
      <PageHeader>
        <SearchInput
          className="sm:max-w-sm"
          clearLabel={t("actions.clear")}
          label={t("panes.logs.search")}
          ref={searchRef}
          value={search}
          onChange={(event) => onSearchChange(event.target.value)}
          onClear={() => onSearchChange("")}
        />
        <Select
          value={filter}
          onValueChange={(value) => {
            if (value === "standard" || value === "issues" || value === "all")
              onFilterChange(value);
          }}
        >
          <SelectTrigger
            className="h-9 min-w-28 shrink-0"
            aria-label={t("panes.logs.levelFilterLabel")}
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="standard">{t("panes.logs.standard")}</SelectItem>
            <SelectItem value="issues">{t("panes.logs.issues")}</SelectItem>
            <SelectItem value="all">{t("panes.logs.all")}</SelectItem>
          </SelectContent>
        </Select>
        <MoreMenu className="ms-auto" label={t("proxy.moreActions")} title={t("proxy.moreActions")}>
          <MenubarItem
            disabled={!filtered.length}
            onSelect={() => void copyShown()}
          >
            <ClipboardCopy className="size-4" aria-hidden="true" />
            {t("panes.logs.copy")}
          </MenubarItem>
          <MenubarItem
            disabled={!filtered.length}
            onSelect={() => void exportShown()}
          >
            <FileDown className="size-4" aria-hidden="true" />
            {t("panes.logs.export")}
          </MenubarItem>
          <MenubarItem disabled={!logLines.length} onSelect={clearLogs}>
            <Trash2 className="size-4" aria-hidden="true" />
            {t("panes.logs.clearDisplay")}
          </MenubarItem>
        </MoreMenu>
      </PageHeader>
      <div className="relative min-h-0 flex-1">
        <div
          className="h-full overflow-y-auto"
          onScroll={handleScroll}
          ref={viewportRef}
          data-testid="logs-viewport"
        >
          {!logLines.length ? (
            <EmptyState
              className="h-full content-center"
              icon={ScrollText}
              title={t("panes.logs.empty")}
            />
          ) : !filtered.length ? (
            <EmptyState
              className="h-full content-center"
              icon={Search}
              title={t("panes.logs.noMatches")}
              description={t("panes.logs.noMatchesDescription")}
            />
          ) : (
            <ol
              className="relative w-full text-xs"
              data-testid="log-lines"
              style={{ height: virtualizer.getTotalSize() }}
            >
              {renderedRows.map(({ index, start }) => {
                const line = filtered[index];
                if (!line) return null;
                return (
                  <li
                    className="absolute inset-x-0"
                    data-testid="log-line"
                    key={line.id}
                    style={{
                      height: ROW_HEIGHT,
                      transform: `translateY(${start}px)`,
                    }}
                  >
                    <button
                      type="button"
                      className="grid h-full w-full grid-cols-[4.25rem_4.5rem_minmax(0,1fr)] items-center gap-3 px-4 text-start outline-none hover:bg-accent-blue-light focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
                      onClick={(event) => {
                        returnFocusRef.current = event.currentTarget;
                        setSelected(line);
                      }}
                    >
                      <time className="tabular-nums text-muted-foreground">
                        {formatTimeOfDay(line.loggedAt)}
                      </time>
                      <Badge
                        className={cn(
                          "h-5 justify-center rounded-sm px-1.5 font-normal",
                          logLevelClassName(line.level),
                        )}
                        variant="outline"
                      >
                        {levels[line.level]}
                      </Badge>
                      <span className="truncate font-mono text-foreground">
                        {line.text}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ol>
          )}
        </div>
        {!atBottom && filtered.length > 0 ? (
          <Button
            className="absolute bottom-4 end-4 gap-2 shadow-overlay"
            onClick={scrollToLatest}
            size="sm"
            type="button"
          >
            <ArrowDownToLine className="size-4" aria-hidden="true" />
            {t("panes.logs.jumpToLatest")}
          </Button>
        ) : null}
      </div>
      <div className="shrink-0 px-4 py-2 text-xs tabular-nums text-muted-foreground">
        {t("panes.logs.count", {
          count: filtered.length,
          total: logLines.length,
        })}
      </div>
      <Dialog
        open={selected !== null}
        onOpenChange={(open) => {
          if (!open) setSelected(null);
        }}
      >
        <ScrollableDialogContent
          width="35rem"
          closeLabel={t("actions.close")}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            restoreFocus(returnFocusRef.current, searchRef.current);
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("panes.logs.details")}</DialogTitle>
            <DialogDescription>
              {selected
                ? `${formatTimeOfDay(selected.loggedAt)} · ${levels[selected.level]}`
                : ""}
            </DialogDescription>
          </DialogHeader>
          <DialogBody className="select-text whitespace-pre-wrap font-mono text-sm [overflow-wrap:anywhere]">
            {selected ? logLineText(t, selected.body) : ""}
          </DialogBody>
        </ScrollableDialogContent>
      </Dialog>
    </section>
  );
}

function logLevelClassName(level: LogLevel) {
  switch (level) {
    // The same quiet tints as the warning and danger badges elsewhere.
    case "warn":
      return "border-transparent bg-warning-bg text-warning";
    case "error":
      return "border-transparent bg-danger-bg text-danger";
    default:
      return "border-transparent bg-transparent text-muted-foreground";
  }
}
