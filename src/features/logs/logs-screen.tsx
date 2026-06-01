import { ScrollText, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useI18n } from "@/i18n/use-i18n";
import { useRuntimeEventStore } from "@/ipc";
import type { LogLevel } from "@/ipc/bindings";
import { cn } from "@/lib/utils";

export function LogsScreen() {
  const { t } = useI18n();
  const clearLogs = useRuntimeEventStore((state) => state.clearLogs);
  const logLines = useRuntimeEventStore((state) => state.logLines);

  return (
    <section className="flex h-full min-h-0 flex-col" aria-label={t("tabs.logs")}>
      <div className="flex h-12 shrink-0 items-center gap-3 border-b px-4">
        <h2 className="flex items-center gap-2 text-sm font-semibold">
          <ScrollText className="size-4 text-muted-foreground" aria-hidden="true" />
          {t("tabs.logs")}
        </h2>
        <Button className="ms-auto gap-2" onClick={clearLogs} size="sm" type="button" variant="outline">
          <Trash2 className="size-4" aria-hidden="true" />
          {t("actions.clear")}
        </Button>
      </div>

      {logLines.length === 0 ? (
        <div className="grid min-h-0 flex-1 place-items-center p-6">
          <div className="grid justify-items-center gap-3 text-center">
            <div className="flex size-10 items-center justify-center rounded-md border bg-card">
              <ScrollText className="size-5 text-muted-foreground" aria-hidden="true" />
            </div>
            <p className="text-sm font-medium">{t("panes.logs.empty")}</p>
          </div>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto bg-muted/20">
          <ol className="grid gap-px p-2 font-mono text-xs leading-5" data-testid="log-lines">
            {logLines.map((line, index) => (
              <li
                className="grid grid-cols-[4.5rem_minmax(0,1fr)] gap-3 rounded-sm px-2 py-1 hover:bg-background"
                key={`${index}-${line.line}`}
              >
                <span className={cn("uppercase", logLevelClassName(line.level))}>{line.level}</span>
                <span className="break-words text-foreground">{line.line}</span>
              </li>
            ))}
          </ol>
        </div>
      )}
    </section>
  );
}

function logLevelClassName(level: LogLevel) {
  switch (level) {
    case "trace":
    case "debug":
      return "text-muted-foreground";
    case "info":
      return "text-foreground";
    case "warn":
      return "text-amber-700 dark:text-amber-400";
    case "error":
      return "text-destructive";
  }
}
