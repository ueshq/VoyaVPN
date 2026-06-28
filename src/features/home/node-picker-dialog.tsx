import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { LoaderCircle, Search } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useI18n } from "@/i18n/use-i18n";
import { listProfiles, restartCore, setActiveProfile, useRuntimeEventStore } from "@/ipc";
import type { ProfileListItem_Serialize } from "@/ipc/bindings";
import { formatDelay } from "@/lib/formatting";
import { cn, getErrorMessage } from "@/lib/utils";
import { useModalStore } from "@/stores/modal-store";

import { getProtocolLabel } from "@/features/profiles/profile-constants";
import { missingCorePayload, shouldOpenSudoPrompt, statusToCoreState } from "./runtime-action";

/**
 * Node picker shown from the Home "Node" tile. Lists every imported profile
 * (across all subscriptions) with a search box; picking one sets it active and,
 * when the tunnel is already up, restarts the core so the switch applies
 * immediately. Reuses the ProfilesScreen query cache (`["profiles", …]`) and the
 * server-table formatting helpers — no new IPC is introduced.
 */
export function NodePickerDialog() {
  const { t } = useI18n();
  const connected = useRuntimeEventStore((state) => state.coreState?.state === "connected");
  const setCoreState = useRuntimeEventStore((state) => state.setCoreState);
  const openModal = useModalStore((state) => state.openModal);
  const closeTopModal = useModalStore((state) => state.closeTopModal);
  const queryClient = useQueryClient();
  const [filterText, setFilterText] = useState("");
  const [switchingId, setSwitchingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const profilesQuery = useQuery({
    queryFn: () => listProfiles(null, null),
    queryKey: ["profiles", { filter: "" }],
  });

  const profiles = useMemo<ProfileListItem_Serialize[]>(() => {
    const data = profilesQuery.data ?? [];
    const query = filterText.trim().toLowerCase();
    const matched = query
      ? data.filter(
          (item) =>
            item.profile.Remarks.toLowerCase().includes(query) ||
            item.profile.Address.toLowerCase().includes(query),
        )
      : data;

    // Stable sort keeps the imported order, but pins the active node to the top.
    return [...matched].sort((a, b) => Number(b.isActive) - Number(a.isActive));
  }, [profilesQuery.data, filterText]);

  async function handleSelect(indexId: string) {
    if (switchingId !== null) {
      return;
    }

    setSwitchingId(indexId);
    setError(null);
    try {
      await setActiveProfile(indexId);
      if (connected) {
        const status = await restartCore();
        setCoreState(statusToCoreState(status));
      }
      await queryClient.invalidateQueries({ queryKey: ["profiles"] });
      closeTopModal();
    } catch (caught) {
      if (shouldOpenSudoPrompt(caught)) {
        closeTopModal();
        openModal("sudo");

        return;
      }

      const missingCore = missingCorePayload(caught);
      if (missingCore) {
        closeTopModal();
        openModal("missingCore", { missingCore });

        return;
      }

      setError(getErrorMessage(caught));
    } finally {
      setSwitchingId(null);
    }
  }

  const showEmpty = !profilesQuery.isPending && profiles.length === 0;

  return (
    <DialogContent className="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle>{t("home.selectNode")}</DialogTitle>
      </DialogHeader>

      <div className="flex flex-col gap-3 px-6 py-4">
        <div className="relative">
          <Search
            aria-hidden="true"
            className="pointer-events-none absolute start-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            aria-label={t("home.searchNodes")}
            autoFocus
            className="ps-9"
            onChange={(event) => setFilterText(event.target.value)}
            placeholder={t("home.searchNodes")}
            value={filterText}
          />
        </div>

        {error ? <p className="text-sm text-destructive">{error}</p> : null}

        <ScrollArea className="-mx-2 h-[min(60vh,24rem)] px-2">
          <div className="flex flex-col gap-0.5">
            {profiles.map((item) => {
              const indexId = item.profile.IndexId;
              const delay = formatDelay(item.profileEx.Delay);

              return (
                <button
                  className={cn(
                    "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-start transition-colors",
                    "hover:bg-surface-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    "disabled:cursor-not-allowed disabled:opacity-60",
                    item.isActive && "bg-surface-raised",
                  )}
                  disabled={switchingId !== null}
                  key={indexId}
                  onClick={() => void handleSelect(indexId)}
                  type="button"
                >
                  <span aria-hidden="true" className="flex size-2 shrink-0 items-center justify-center">
                    {item.isActive ? <span className="size-1.5 rounded-full bg-connected" /> : null}
                  </span>

                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="flex items-center gap-2">
                      <span className="truncate text-sm font-medium text-foreground">
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

                  {switchingId === indexId ? (
                    <LoaderCircle aria-hidden="true" className="size-4 shrink-0 animate-spin text-muted-foreground" />
                  ) : delay ? (
                    <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{delay}</span>
                  ) : null}
                </button>
              );
            })}

            {showEmpty ? (
              <p className="px-3 py-6 text-center text-sm text-muted-foreground">{t("home.noNodes")}</p>
            ) : null}
          </div>
        </ScrollArea>
      </div>
    </DialogContent>
  );
}
