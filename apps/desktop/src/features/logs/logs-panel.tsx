import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDownToLine,
  ClipboardCopy,
  Ellipsis,
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
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import { EmptyState } from "@voya/ui/components/empty-state";
import { Input } from "@voya/ui/components/input";
import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarTrigger,
} from "@voya/ui/components/menubar";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@voya/ui/components/select";
import { useI18n } from "@voya/i18n/use-i18n";

import { getErrorMessage } from "@voya/utils/error";
import { exportLogs } from "@/ipc/commands";
import { logLineText } from "@/ipc/messages";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import type { StoredLogLine } from "@/ipc/runtime-event-store";
import type { LogLevel } from "@/ipc/bindings";
import { cn } from "@voya/ui/lib/utils";
import { PageHeader } from "@/components/app-shell/page-section";
import { useToastStore } from "@/stores/toast-store";

export type LogFilter = "standard" | "issues" | "all";
const ROW_HEIGHT = 36;
const STICK_THRESHOLD = 24;

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
  const clearLogs = useRuntimeEventStore((state) => state.clearLogs);
  const logLines = useRuntimeEventStore((state) => state.logLines);
  const [selected, setSelected] = useState<StoredLogLine | null>(null);
  const returnFocusRef = useRef<HTMLButtonElement | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const needle = search.trim().toLowerCase();
  const resolved = useMemo(
    () =>
      logLines.map((line) => ({ ...line, text: logLineText(t, line.body) })),
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
          included && (!needle || line.text.toLowerCase().includes(needle))
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
          `${formatTimestamp(line.receivedAt)} [${line.level}] ${line.text}`,
      )
      .join("\n");

  async function copyShown() {
    try {
      if (typeof navigator === "undefined" || !navigator.clipboard?.writeText) {
        throw new Error(t("status.copyTunDiagnosticsClipboardUnavailable"));
      }
      await navigator.clipboard.writeText(shownText());
      pushToast({
        description: t("panes.logs.copied", { count: filtered.length }),
        severity: "info",
        title: t("panes.logs.copy"),
      });
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("panes.logs.copyFailed"),
      });
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
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("panes.logs.exportFailed"),
      });
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
  const virtualRows = virtualizer.getVirtualItems();
  const renderedRows = virtualRows.length
    ? virtualRows
    : filtered
        .slice(0, 50)
        .map((_, index) => ({ index, start: index * ROW_HEIGHT }));

  return (
    <section
      aria-label={t("tabs.logs")}
      className="flex h-full min-h-0 min-w-0 flex-col"
    >
      <PageHeader>
        <div className="relative min-w-0 flex-1 sm:max-w-sm">
          <Search
            className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            aria-label={t("panes.logs.search")}
            className="h-9 ps-9"
            onChange={(event) => onSearchChange(event.target.value)}
            placeholder={t("panes.logs.search")}
            type="search"
            value={search}
            ref={searchRef}
          />
        </div>
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
        <Menubar className="ms-auto h-auto border-0 bg-transparent p-0 shadow-none">
          <MenubarMenu>
            <MenubarTrigger asChild>
              <Button size="sm" type="button" variant="ghost">
                <Ellipsis className="size-4" aria-hidden="true" />
                {t("proxy.moreActions")}
              </Button>
            </MenubarTrigger>
            <MenubarContent align="end">
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
            </MenubarContent>
          </MenubarMenu>
        </Menubar>
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
                        {formatTimestamp(line.receivedAt)}
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
            className="absolute bottom-4 end-4 gap-2 shadow-md"
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
        <DialogContent
          className="flex max-h-[85vh] flex-col sm:max-w-[560px]"
          closeLabel={t("actions.close")}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            (returnFocusRef.current?.isConnected
              ? returnFocusRef.current
              : searchRef.current
            )?.focus();
          }}
        >
          <DialogHeader>
            <DialogTitle>{t("panes.logs.details")}</DialogTitle>
            <DialogDescription>
              {selected
                ? `${formatTimestamp(selected.receivedAt)} · ${levels[selected.level]}`
                : ""}
            </DialogDescription>
          </DialogHeader>
          <DialogBody className="select-text whitespace-pre-wrap font-mono text-sm [overflow-wrap:anywhere]">
            {selected ? logLineText(t, selected.body) : ""}
          </DialogBody>
        </DialogContent>
      </Dialog>
    </section>
  );
}

function formatTimestamp(receivedAt: number) {
  const date = new Date(receivedAt);
  const pad = (value: number) => value.toString().padStart(2, "0");
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

function logLevelClassName(level: LogLevel) {
  switch (level) {
    case "warn":
      return "border-warning-bold/30 bg-warning-bg text-warning";
    case "error":
      return "border-destructive/30 bg-destructive/10 text-danger";
    default:
      return "border-transparent bg-transparent text-muted-foreground";
  }
}
