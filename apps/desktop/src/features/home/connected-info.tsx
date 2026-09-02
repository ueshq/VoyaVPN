import { ArrowDown, ArrowUp, RotateCw } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { useRuntimeEventStore } from "@/ipc";
import { useShellStore } from "@/stores/shell-store";
import { formatBytesPerSecond, formatDelay } from "@voya/utils/formatting";

import type { ActiveNodeInfo, Translation } from "./use-home-runtime";

/**
 * Post-connect summary under the big button: the running node (a link to the
 * Proxies page), its measured delay, live up/down speeds, and Restart.
 */
export function ConnectedInfo({
  activeNode,
  busy,
  onRestart,
  t,
}: {
  activeNode: ActiveNodeInfo | null;
  busy: boolean;
  onRestart: () => void;
  t: Translation;
}) {
  const statistics = useRuntimeEventStore((state) => state.statistics);
  const requestTab = useShellStore((state) => state.requestTab);
  const uploadLabel = t("status.upload", {
    speed: formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0),
  });
  const downloadLabel = t("status.download", {
    speed: formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0),
  });

  return (
    <div className="grid justify-items-center gap-1.5" data-testid="home-connected-info">
      <div className="flex items-center gap-2">
        {activeNode ? (
          <Button
            aria-label={t("home.currentNode", { node: activeNode.name })}
            className="h-7 max-w-64 gap-1.5 px-2"
            onClick={() => requestTab("proxies")}
            size="sm"
            type="button"
            variant="ghost"
          >
            <span aria-hidden="true" className="size-1.5 shrink-0 rounded-full bg-connected" />
            <span className="truncate text-sm font-medium">{activeNode.name}</span>
            {activeNode.delayMs != null ? (
              <span className="text-xs tabular-nums text-muted-foreground">
                {formatDelay(activeNode.delayMs)}
              </span>
            ) : null}
          </Button>
        ) : null}
        <Button
          aria-label={t("actions.restart")}
          className="size-7"
          disabled={busy}
          onClick={onRestart}
          size="icon"
          type="button"
          variant="ghost"
        >
          <RotateCw aria-hidden="true" className="size-4" />
        </Button>
      </div>
      <p className="flex items-center gap-3 text-xs tabular-nums text-muted-foreground">
        <span className="inline-flex items-center gap-1">
          <ArrowUp aria-hidden="true" className="size-3" />
          {uploadLabel}
        </span>
        <span className="inline-flex items-center gap-1">
          <ArrowDown aria-hidden="true" className="size-3" />
          {downloadLabel}
        </span>
      </p>
    </div>
  );
}
