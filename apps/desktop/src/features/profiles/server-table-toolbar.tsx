import { useRef, useState } from "react";
import {
  Check,
  ChevronDown,
  FilePlus2,
  Layers,
  Plus,
  RefreshCw,
  Rss,
  Settings2,
  SlidersHorizontal,
} from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { DisabledReason } from "@/components/disabled-reason";
import {
  Menubar,
  MenubarContent,
  MenubarMenu,
  MenubarItem,
  MenubarSeparator,
  MenubarTrigger,
} from "@voya/ui/components/menubar";
import { cn } from "@voya/ui/lib/utils";
import { useShellStore } from "@/stores/shell-store";

import { IMPORT_METHODS } from "@voya/features/profiles/import-methods";
import { IMPORT_METHOD_ICONS } from "./import-methods";
import { SpeedtestButton } from "./server-table-menus";
import { SpeedtestSettingsDialog } from "./speedtest-settings-dialog";
import type { ServerTableController } from "./use-server-table";

export function ServerTableToolbar({
  controller,
}: {
  controller: ServerTableController;
}) {
  const {
    handleCancelSpeedtest,
    handleDirectImport,
    handleSpeedtest,
    directImportPending,
    nodeGroups,
    profiles,
    speedtestProgress,
    speedtestRunning,
    speedtestSource,
    addTriggerRef,
    setDialogState,
    setImportMethod,
    openGroupEditor,
    openSubscription,
    t,
    updateAllSubscriptions,
    updatingAllSubscriptions,
  } = controller;
  const addMenuOpen = useShellStore((state) => state.profilesAddMenuOpen);
  const focusFirstAddItemRef = useRef(addMenuOpen);
  const firstAddItemRef = useRef<HTMLDivElement>(null);
  const openingDialogRef = useRef(false);
  const [speedtestSettingsOpen, setSpeedtestSettingsOpen] = useState(false);
  function handleMenuClose(event: Event) {
    if (openingDialogRef.current) {
      event.preventDefault();
      openingDialogRef.current = false;
    }
  }

  return (
    <div
      className="flex flex-wrap items-center gap-2 min-w-0 max-w-full justify-end"
      data-slot="toolbar"
      role="toolbar"
    >
      <DisabledReason
        reason={profiles.length ? undefined : t("panes.profiles.speedtest.nothingToTest")}
      >
        {/* A run restored from an earlier launch has no known owner, so Test all can stop it. */}
        <SpeedtestButton
          busyElsewhere={speedtestRunning && speedtestSource !== null && speedtestSource !== "all"}
          disabled={!profiles.length}
          label={t("panes.profiles.speedtest.testAll")}
          onCancel={handleCancelSpeedtest}
          onRun={() =>
            handleSpeedtest(
              {
                profileIds: profiles.map((item) => item.profile.id),
                scope: "profiles",
              },
              "all",
            )
          }
          progress={speedtestProgress}
          running={speedtestRunning && (speedtestSource === null || speedtestSource === "all")}
        />
      </DisabledReason>
      <Menubar bare>
        <MenubarMenu>
          <MenubarTrigger asChild className="h-8">
            <Button size="sm" type="button" variant="outline">
              <SlidersHorizontal className="size-4" aria-hidden="true" />
              {t("panes.profiles.view.label")}
              <ChevronDown className="size-3" aria-hidden="true" />
            </Button>
          </MenubarTrigger>
          <MenubarContent align="end" onCloseAutoFocus={handleMenuClose}>
            <MenubarItem
              aria-checked={nodeGroups.sortByLatency}
              onSelect={() => nodeGroups.setSortByLatency(!nodeGroups.sortByLatency)}
              role="menuitemcheckbox"
            >
              <Check
                aria-hidden="true"
                className={cn(!nodeGroups.sortByLatency && "invisible")}
              />
              {t("panes.profiles.view.sortByLatency")}
            </MenubarItem>
            <MenubarItem
              aria-checked={nodeGroups.hideUnreachable}
              onSelect={() => nodeGroups.setHideUnreachable(!nodeGroups.hideUnreachable)}
              role="menuitemcheckbox"
            >
              <Check
                aria-hidden="true"
                className={cn(!nodeGroups.hideUnreachable && "invisible")}
              />
              {t("panes.profiles.view.hideUnreachable")}
            </MenubarItem>
            <MenubarSeparator />
            <MenubarItem
              onSelect={() => {
                openingDialogRef.current = true;
                setSpeedtestSettingsOpen(true);
              }}
            >
              <Settings2 aria-hidden="true" />
              {t("panes.profiles.speedtest.settings")}
            </MenubarItem>
          </MenubarContent>
        </MenubarMenu>
      </Menubar>
      <Button
        disabled={updatingAllSubscriptions}
        onClick={() => void updateAllSubscriptions()}
        size="sm"
        type="button"
        variant="outline"
      >
        <RefreshCw
          aria-hidden="true"
          className={cn("size-4", updatingAllSubscriptions && "animate-spin")}
        />
        {t("panes.profiles.toolbar.updateAllSubscriptions")}
      </Button>
      <Menubar
        bare
        value={addMenuOpen ? "add" : ""}
        onValueChange={(value) => {
          useShellStore.getState().setProfilesAddMenuOpen(value === "add");
        }}
      >
        <MenubarMenu value="add">
          <MenubarTrigger asChild className="h-8">
            <Button ref={addTriggerRef} size="sm" type="button">
              <Plus className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.add")}
              <ChevronDown className="size-3" aria-hidden="true" />
            </Button>
          </MenubarTrigger>
          <MenubarContent
            onCloseAutoFocus={handleMenuClose}
            onFocus={(event) => {
              // Radix focuses the container for a programmatic opening.
              // The Home guide should land on the first available action.
              if (focusFirstAddItemRef.current && event.target === event.currentTarget) {
                focusFirstAddItemRef.current = false;
                firstAddItemRef.current?.focus();
              }
            }}
          >
            {IMPORT_METHODS.map(({ method, labelKey }, index) => {
              const Icon = IMPORT_METHOD_ICONS[method];
              return (
              <MenubarItem
                key={method}
                disabled={directImportPending !== null}
                ref={index === 0 ? firstAddItemRef : undefined}
                onSelect={() => {
                  if (method === "clipboard" || method === "qrScreen") {
                    void handleDirectImport(method);
                  } else {
                    openingDialogRef.current = true;
                    setImportMethod(method);
                  }
                }}
              >
                <Icon aria-hidden="true" />
                {t(labelKey)}
              </MenubarItem>
              );
            })}
            <MenubarSeparator />
            <MenubarItem onSelect={() => {
              openingDialogRef.current = true;
              openSubscription(null, addTriggerRef.current ?? undefined);
            }}>
              <Rss aria-hidden="true" />
              <span className="grid">
                <span>{t("home.subscriptionCard.add")}</span>
                {/* Sets it apart from pasting a subscription link above. */}
                <span aria-hidden="true" className="text-xs text-muted-foreground">
                  {t("home.subscriptionCard.addHint")}
                </span>
              </span>
            </MenubarItem>
            <MenubarItem onSelect={() => {
              openingDialogRef.current = true;
              setDialogState({ mode: "create" });
            }}>
              <FilePlus2 aria-hidden="true" />
              {t("panes.profiles.toolbar.manualNode")}
            </MenubarItem>
            <MenubarItem onSelect={() => {
              openingDialogRef.current = true;
              openGroupEditor(null);
            }}>
              <Layers aria-hidden="true" />
              {t("policyGroups.new")}
            </MenubarItem>
          </MenubarContent>
        </MenubarMenu>
      </Menubar>
      {speedtestSettingsOpen ? (
        <SpeedtestSettingsDialog onOpenChange={setSpeedtestSettingsOpen} />
      ) : null}
    </div>
  );
}
