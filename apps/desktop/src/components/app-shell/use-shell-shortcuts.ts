import { useEffect } from "react";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";

import type { TranslationFunction } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { runRuntimeAction, runtimeBusy } from "@/stores/runtime-action";
import type { ProfileListing } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { coreStateOf, useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { SHELL_TABS, useShellStore } from "@/stores/shell-store";

function isApplePlatform() {
  if (typeof navigator === "undefined") return false;
  return /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
}

/** How a page shortcut reads next to its tab, such as ⌘2 on macOS. */
export function pageShortcutLabel(index: number, t: TranslationFunction) {
  return t(isApplePlatform() ? "shortcuts.pageMac" : "shortcuts.page", { number: index + 1 });
}

/** The same shortcut in `aria-keyshortcuts` form. */
export function pageShortcutAria(index: number) {
  return `${isApplePlatform() ? "Meta" : "Control"}+${index + 1}`;
}

/** How the connect shortcut reads on the connect button. */
export function connectionShortcutLabel(t: TranslationFunction) {
  return t(isApplePlatform() ? "shortcuts.connectMac" : "shortcuts.connect");
}

/**
 * App-wide keys. The platform modifier with 1–5 opens a page in sidebar order,
 * and with Shift+C connects or disconnects from whichever page is showing.
 */
export function useShellShortcuts() {
  const { t } = useI18n();
  const queryClient = useQueryClient();

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const modifier = isApplePlatform() ? event.metaKey : event.ctrlKey;
      if (!modifier || event.altKey || event.defaultPrevented) return;
      if (event.shiftKey) {
        if (event.key.toLowerCase() === "c") {
          event.preventDefault();
          void toggleConnection(t, queryClient);
        }
        return;
      }
      const tab = /^[1-9]$/.test(event.key) ? SHELL_TABS[Number(event.key) - 1] : undefined;
      if (tab) {
        event.preventDefault();
        useShellStore.getState().setActiveTab(tab, true);
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [queryClient, t]);
}

/** What the Home connect button does, from any page. */
async function toggleConnection(t: TranslationFunction, queryClient: QueryClient) {
  const state = coreStateOf(useRuntimeEventStore.getState().coreState);
  if (runtimeBusy(state)) return;
  const action = state === "connected" || state === "cleanupPending" ? "disconnect" : "connect";
  const profiles = queryClient.getQueryData<ProfileListing>(queryKeys.profileList);
  if (action === "connect" && profiles?.entries.length === 0) {
    // Nothing to connect with: go where a node gets added, as Home's button does.
    useShellStore.getState().openProfilesAddMenu();
    return;
  }

  // Home shows a failure next to its button; other pages need a toast.
  await runRuntimeAction(action, t, { inline: useShellStore.getState().activeTab === "home" });
}
