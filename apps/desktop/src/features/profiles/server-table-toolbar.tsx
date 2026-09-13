import { useRef } from "react";
import {
  ChevronDown,
  FilePlus2,
  Plus,
  RefreshCw,
  Rss,
  Upload,
} from "lucide-react";

import { Toolbar } from "@/components/app-shell/toolbar";
import { Button } from "@voya/ui/components/button";
import {
  Menubar,
  MenubarContent,
  MenubarMenu,
  MenubarItem,
  MenubarTrigger,
} from "@voya/ui/components/menubar";
import { useShellStore } from "@/stores/shell-store";

import { IMPORT_METHODS } from "./import-methods";
import type { NodeToolbarController } from "./node-controller-types";

export function ServerTableToolbar({
  controller,
}: {
  controller: NodeToolbarController;
}) {
  const {
    handleDirectImport,
    directImportPending,
    addTriggerRef,
    importTriggerRef,
    setDialogState,
    setImportMethod,
    openSubscription,
    t,
    updateAllSubscriptions,
    updatingAllSubscriptions,
  } = controller;
  const addMenuOpen = useShellStore((state) => state.profilesAddMenuOpen);
  const focusFirstAddItemRef = useRef(addMenuOpen);
  const addNodeItemRef = useRef<HTMLDivElement>(null);
  const openingDialogRef = useRef(false);
  function handleMenuClose(event: Event) {
    if (openingDialogRef.current) {
      event.preventDefault();
      openingDialogRef.current = false;
    }
  }

  return (
    <Toolbar className="min-w-0 max-w-full justify-end">
      <Menubar
        className="h-auto border-0 bg-transparent p-0 shadow-none"
        value={addMenuOpen ? "add" : ""}
        onValueChange={(value) => {
          useShellStore.setState({ profilesAddMenuOpen: value === "add" });
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
                addNodeItemRef.current?.focus();
              }
            }}
          >
            <MenubarItem ref={addNodeItemRef} onSelect={() => {
              openingDialogRef.current = true;
              setDialogState({ mode: "create" });
            }}>
              <FilePlus2 aria-hidden="true" />
              {t("panes.profiles.toolbar.addNode")}
            </MenubarItem>
            <MenubarItem onSelect={() => {
              openingDialogRef.current = true;
              openSubscription(null, addTriggerRef.current ?? undefined);
            }}>
              <Rss aria-hidden="true" />
              {t("home.subscriptionCard.add")}
            </MenubarItem>
            <MenubarItem
              disabled={updatingAllSubscriptions}
              onSelect={() => void updateAllSubscriptions()}
            >
              <RefreshCw aria-hidden="true" />
              {t("panes.profiles.toolbar.updateAllSubscriptions")}
            </MenubarItem>
          </MenubarContent>
        </MenubarMenu>
      </Menubar>
      <Menubar className="h-auto border-0 bg-transparent p-0 shadow-none">
        <MenubarMenu>
          <MenubarTrigger asChild className="h-8">
            <Button
              ref={importTriggerRef}
              disabled={directImportPending !== null}
              size="sm"
              type="button"
              variant="outline"
            >
              <Upload className="size-4" aria-hidden="true" />
              {t("panes.profiles.toolbar.import")}
              <ChevronDown className="size-3" aria-hidden="true" />
            </Button>
          </MenubarTrigger>
          <MenubarContent onCloseAutoFocus={handleMenuClose}>
            {IMPORT_METHODS.map(({ method, icon: Icon, labelKey }) => (
              <MenubarItem
                key={method}
                disabled={directImportPending !== null}
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
            ))}
          </MenubarContent>
        </MenubarMenu>
      </Menubar>
    </Toolbar>
  );
}
