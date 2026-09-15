import { useMemo, useState } from "react";
import { AppWindow, Info, Plus, X } from "lucide-react";
import { useQuery } from "@tanstack/react-query";

import type { TranslationKey } from "@voya/i18n";

import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { Checkbox } from "@voya/ui/components/checkbox";
import {
  Dialog,
  DialogDescription,
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { EmptyState } from "@voya/ui/components/empty-state";
import { Input } from "@voya/ui/components/input";
import { Label } from "@voya/ui/components/label";
import { ScrollArea } from "@voya/ui/components/scroll-area";
import { SegmentedControl, SegmentedControlItem } from "@voya/ui/components/segmented-control";
import { Spinner } from "@voya/ui/components/spinner";
import {
  connectionModeStatus,
  deleteRoutingRules,
  listProcessCandidates,
  listRoutings,
  moveRoutingRule,
  saveRoutingRule,
} from "@/ipc/commands";
import { queryKeys } from "@/ipc/query-keys";
import { useDialogSubmit } from "@/lib/use-dialog-submit";
import { useI18n } from "@voya/i18n/use-i18n";

import {
  buildPerAppRule,
  findPerAppRule,
  normalizeProcessNames,
  PER_APP_MODE_LABEL_KEYS,
  readPerAppRule,
  type PerAppProxyMode,
} from "./per-app-proxy-rule";

type PerAppProxyDialogProps = {
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

const MODE_OPTIONS: PerAppProxyMode[] = ["off", "include", "exclude"];

const MODE_HINT_KEYS = {
  exclude: "panes.routing.perAppModeExcludeHint",
  include: "panes.routing.perAppModeIncludeHint",
  off: "panes.routing.perAppModeOffHint",
} as const satisfies Record<PerAppProxyMode, TranslationKey>;

/**
 * Visual editor for per-app proxy rules. It manages exactly one sentinel rule
 * in the active routing set (include → always proxy, exclude → always direct),
 * fed by an OS process picker plus manual entry. sing-box process rules only
 * match TUN traffic, so a hint appears when the current mode is not VPN.
 */
export function PerAppProxyDialog({
  onOpenChange,
  open,
}: PerAppProxyDialogProps) {
  const { t } = useI18n();
  const { error, pending: saving, setError, submit } = useDialogSubmit();
  const [manualEntry, setManualEntry] = useState("");
  const [mode, setMode] = useState<PerAppProxyMode>("off");
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  // Tracks whether the form was seeded for the current open cycle.
  const [seeded, setSeeded] = useState(false);

  const routingsQuery = useQuery({
    enabled: open,
    queryFn: listRoutings,
    queryKey: queryKeys.routings,
  });
  const candidatesQuery = useQuery({
    enabled: open,
    queryFn: listProcessCandidates,
    queryKey: queryKeys.processCandidates,
    staleTime: 30_000,
  });
  const modeStatusQuery = useQuery({
    enabled: open,
    queryFn: connectionModeStatus,
    queryKey: queryKeys.connectionMode,
  });

  const activeRouting =
    routingsQuery.data?.find((routing) => routing.isActive) ?? null;

  // Seed the form from the active routing once per open cycle. Adjusting state
  // during render (React's documented pattern) instead of in an effect avoids
  // a cascading-render lint and an extra paint.
  if (!open && seeded) {
    setSeeded(false);
  }
  if (open && !seeded && routingsQuery.data != null) {
    const state = readPerAppRule(activeRouting);
    setMode(state.mode);
    setSelected(state.processes);
    setManualEntry("");
    setSearch("");
    setError(null);
    setSeeded(true);
  }

  const candidatesData = candidatesQuery.data;
  const candidates = useMemo(() => candidatesData ?? [], [candidatesData]);
  const filteredCandidates = useMemo(() => {
    const needle = search.trim().toLowerCase();
    if (!needle) {
      return candidates;
    }
    return candidates.filter(
      (candidate) =>
        candidate.displayName.toLowerCase().includes(needle) ||
        candidate.processName.toLowerCase().includes(needle),
    );
  }, [candidates, search]);
  const selectedKeys = useMemo(
    () => new Set(selected.map((process) => process.toLowerCase())),
    [selected],
  );

  function toggleProcess(processName: string) {
    setSelected((current) => {
      const key = processName.toLowerCase();
      if (current.some((process) => process.toLowerCase() === key)) {
        return current.filter((process) => process.toLowerCase() !== key);
      }
      return [...current, processName];
    });
  }

  function addManualEntry() {
    const normalized = normalizeProcessNames([...selected, manualEntry]);
    setSelected(normalized);
    setManualEntry("");
  }

  async function handleSave() {
    if (!activeRouting || saving) {
      return;
    }
    await submit(async () => {
        const existing = findPerAppRule(activeRouting);
        const processes = normalizeProcessNames(selected);
        if (mode === "off") {
          // Turning it off keeps the chosen apps on a disabled rule, so turning
          // it back on does not mean picking them all again.
          if (existing && processes.length > 0) {
            await saveRoutingRule(activeRouting.id, {
              ...existing,
              enabled: false,
              process: processes,
            });
          } else if (existing) {
            await deleteRoutingRules(activeRouting.id, [existing.id]);
          }
        } else {
          const saved = await saveRoutingRule(
            activeRouting.id,
            buildPerAppRule(mode, processes, existing),
          );
          // The backend appends a new rule to the end of the rule set, which puts
          // it behind the catch-all rule every built-in routing ends with — a
          // process rule there can never match. Pin the managed rule to the top so
          // the listed apps really do take precedence, as documented in
          // per-app-proxy-rule.ts.
          const savedRule = findPerAppRule(saved);
          if (savedRule && saved.rules[0]?.id !== savedRule.id) {
            await moveRoutingRule(saved.id, savedRule.id, "top", null);
          }
        }
        // The routing-rule commands emit the `routings` invalidation.
        onOpenChange(false);
    });
  }

  const vpnHintProminent =
    modeStatusQuery.data?.processRulesEffective === false;
  // Turning the rule on without any app would save nothing; say so instead.
  const missingApps = mode !== "off" && normalizeProcessNames(selected).length === 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <ScrollableDialogContent closeLabel={t("actions.close")} width="54rem">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <AppWindow className="size-4" aria-hidden="true" />
            {t("panes.routing.perAppTitle")}
          </DialogTitle>
          <DialogDescription className="sr-only">
            {t("panes.routing.perAppDescription")}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          {activeRouting == null && !routingsQuery.isLoading ? (
            <Alert>
              <AlertDescription>
                {t("panes.routing.perAppNoActiveRouting")}
              </AlertDescription>
            </Alert>
          ) : (
            <div className="grid gap-4">
              <SegmentedControl aria-label={t("panes.routing.perAppTitle")}>
                {MODE_OPTIONS.map((option) => (
                  <SegmentedControlItem
                    key={option}
                    onClick={() => setMode(option)}
                    pressed={mode === option}
                  >
                    {t(PER_APP_MODE_LABEL_KEYS[option])}
                  </SegmentedControlItem>
                ))}
              </SegmentedControl>
              <p className="text-xs text-muted-foreground">
                {t(MODE_HINT_KEYS[mode])}
              </p>

              {/* Only worth saying when the running mode cannot apply app rules. */}
              {vpnHintProminent ? (
                <Alert>
                  <Info aria-hidden="true" className="size-4" />
                  <AlertDescription>{t("panes.routing.perAppTunOnlyHint")}</AlertDescription>
                </Alert>
              ) : null}

              {mode !== "off" ? (
                <>
                  {selected.length > 0 ? (
                    <div className="flex flex-wrap gap-1.5">
                      {selected.map((process) => (
                        <Badge
                          className="gap-1 pe-1"
                          key={process.toLowerCase()}
                          variant="secondary"
                        >
                          <span className="max-w-48 truncate">{process}</span>
                          <Button
                            aria-label={t("panes.routing.perAppRemove", {
                              process,
                            })}
                            // A 16px glyph with a 24px hit area, so the badge keeps its height.
                            className="relative size-4 rounded-full after:absolute after:-inset-1"
                            onClick={() => toggleProcess(process)}
                            size="icon-sm"
                            type="button"
                            variant="ghost"
                          >
                            <X aria-hidden="true" className="size-3" />
                          </Button>
                        </Badge>
                      ))}
                    </div>
                  ) : null}

                  <div className="grid gap-2">
                    <Label
                      className="text-xs text-muted-foreground"
                      htmlFor="per-app-search"
                    >
                      {t("panes.routing.perAppRunningApps")}
                    </Label>
                    <Input
                      className="bg-card"
                      id="per-app-search"
                      onChange={(event) => setSearch(event.target.value)}
                      placeholder={t("panes.routing.perAppPickerSearch")}
                      value={search}
                    />
                    <ScrollArea className="h-56 rounded-md border bg-card">
                      {candidatesQuery.isLoading ? (
                        <p
                          className="p-3 text-xs text-muted-foreground"
                          role="status"
                        >
                          {t("panes.routing.perAppLoading")}
                        </p>
                      ) : filteredCandidates.length === 0 ? (
                        <EmptyState
                          description={t(
                            "panes.routing.perAppEmptyDescription",
                          )}
                          icon={AppWindow}
                          title={t("panes.routing.perAppEmpty")}
                        />
                      ) : (
                        <div className="p-1">
                          {filteredCandidates.map((candidate) => {
                            const checked = selectedKeys.has(
                              candidate.processName.toLowerCase(),
                            );
                            const checkboxId = `per-app-${candidate.processName.toLowerCase()}`;

                            return (
                              <Label
                                className="flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-accent hover:text-accent-foreground"
                                htmlFor={checkboxId}
                                key={candidate.processName.toLowerCase()}
                              >
                                <Checkbox
                                  checked={checked}
                                  id={checkboxId}
                                  onCheckedChange={() =>
                                    toggleProcess(candidate.processName)
                                  }
                                />
                                <span className="min-w-0 flex-1">
                                  <span className="block truncate font-medium">
                                    {candidate.displayName}
                                  </span>
                                  {candidate.processName !==
                                    candidate.displayName ||
                                  candidate.executablePath ? (
                                    <span className="block truncate text-xs text-muted-foreground">
                                      {candidate.executablePath ??
                                        candidate.processName}
                                    </span>
                                  ) : null}
                                </span>
                              </Label>
                            );
                          })}
                        </div>
                      )}
                    </ScrollArea>
                  </div>

                  <div className="flex items-end gap-2">
                    <div className="grid min-w-0 flex-1 gap-1">
                      <Label
                        className="text-xs text-muted-foreground"
                        htmlFor="per-app-manual"
                      >
                        {t("panes.routing.perAppManualAdd")}
                      </Label>
                      <Input
                        className="bg-card"
                        id="per-app-manual"
                        onChange={(event) => setManualEntry(event.target.value)}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            event.preventDefault();
                            addManualEntry();
                          }
                        }}
                        value={manualEntry}
                      />
                    </div>
                    <Button
                      disabled={manualEntry.trim().length === 0}
                      onClick={addManualEntry}
                      type="button"
                      variant="outline"
                    >
                      <Plus aria-hidden="true" className="size-4" />
                      {t("actions.add")}
                    </Button>
                  </div>
                </>
              ) : null}

              {missingApps ? (
                <p className="text-sm text-warning" role="status">
                  {t("panes.routing.perAppNeedsApps")}
                </p>
              ) : null}
              {error ? (
                <Alert variant="destructive">
                  <AlertDescription>{error}</AlertDescription>
                </Alert>
              ) : null}
            </div>
          )}
        </DialogBody>
        <DialogFooter>
          <Button
            onClick={() => onOpenChange(false)}
            type="button"
            variant="outline"
          >
            {t("actions.cancel")}
          </Button>
          <Button
            disabled={activeRouting == null || saving || missingApps}
            onClick={() => void handleSave()}
            type="button"
          >
            {saving ? <Spinner className="size-4" /> : null}
            {t("actions.save")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}
