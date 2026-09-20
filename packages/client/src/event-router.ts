import type { QueryClient } from "@tanstack/react-query";

import type {
  AppEvent,
  InvalidateEvent,
  NoticeCode,
  ShellTabTarget,
  TransientStreamEvent,
} from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n/core";
import { getErrorMessage } from "@voya/utils/error";

import { noticeText } from "./messages";
import { invalidationQueryKey, queryKeys } from "./query-keys";
import { useRuntimeEventStore } from "./runtime-event-store";
import { useToastStore } from "./toast-store";

/**
 * Notices a user must not miss even with the app out of sight: the connection
 * stopping, the node or group in use disappearing, a tray action failing, a
 * subscription that stopped updating. The rest are only toasts.
 */
const BACKGROUND_NOTICE_CODES = new Set<NoticeCode["code"]>([
  "activeSelectionRemoved",
  "coreStopped",
  "nativeTunStopped",
  "subscriptionAutoUpdateFailed",
  "trayActionFailed",
]);

/** Longest a finished speedtest result waits for its country flag to show. */
const COUNTRY_REFRESH_MS = 3000;

/**
 * Where a `selectTab` deep link points, with the wire name normalized.
 *
 * `proxyConnections` is one page with two sub-views on the desktop and its own
 * tab on mobile, and the runtime log lives under Settings → Advanced on the
 * desktop but is a page of its own elsewhere. The router says *what* was asked
 * for; each app decides how to get there.
 */
export type ShellTarget =
  | { tab: "logs" }
  | { tab: "profiles" }
  | { tab: "connections"; view: "connections" };

export type EventRouterOptions = {
  queryClient: QueryClient;
  /** Read per call, so a language change reaches the next notice. */
  t: () => TranslationFunction;
  onSelectTab: (target: ShellTarget) => void;
  onCloseRequested: () => void;
  /**
   * Repeats a background-worthy notice where the user will see it while the
   * app is not on screen. Optional: a platform without one simply toasts.
   */
  notify?: (title: string) => void;
};

export type EventRouter = {
  onInvalidate: (event: InvalidateEvent) => void;
  onAppEvent: (event: AppEvent) => void;
  onTransient: (event: TransientStreamEvent) => void;
  dispose: () => void;
};

/**
 * Turns the three backend channels into store writes and cache invalidations.
 *
 * Everything here is transport-agnostic: a platform bridge subscribes however
 * it can — Tauri listeners, a native module's event emitter — and forwards the
 * decoded payloads. What each event *means* is decided once, here.
 */
export function createEventRouter(options: EventRouterOptions): EventRouter {
  let countryRefresh: ReturnType<typeof setTimeout> | undefined;

  // A renderer restored mid-run has no pending run promise of its own, so the
  // running state is read back rather than waited for.
  void useRuntimeEventStore
    .getState()
    .refreshSpeedtestStatus()
    .catch((error: unknown) => {
      reportEventRouterError("failed to refresh speedtest status", error);
    });

  function onInvalidate(event: InvalidateEvent) {
    const runtime = useRuntimeEventStore.getState();
    if (event.keys.some((item) => item.scope.kind === "profiles")) {
      // Saved profiles supersede old measurements, including pending markers
      // from a test whose connection snapshot became obsolete during an edit.
      runtime.clearSpeedtestResults();
      if (runtime.speedtestRunning) {
        void runtime.refreshSpeedtestStatus().catch((error: unknown) => {
          reportEventRouterError("failed to refresh speedtest status", error);
        });
      }
    }
    event.keys.forEach((item) => {
      // `null` only for a scope this build cannot map, which `check:bindings`
      // makes impossible; skipping beats throwing inside the event callback.
      const queryKey = invalidationQueryKey(item.scope);
      if (queryKey) {
        void options.queryClient.invalidateQueries({ queryKey });
      }
    });
  }

  function onAppEvent(event: AppEvent) {
    switch (event.kind) {
      case "notice": {
        // `detail` is the untranslated diagnostic behind the notice; the title
        // is resolved from the code against the current locale.
        const title = noticeText(options.t(), event.payload.code);
        useToastStore.getState().pushToast({
          description: event.payload.detail ?? undefined,
          severity: event.payload.level,
          title,
        });
        // A toast reaches nobody while the app is out of sight. The OS
        // notification carries the title only, because the detail is untranslated.
        if (BACKGROUND_NOTICE_CODES.has(event.payload.code.code)) {
          options.notify?.(title);
        }
        return;
      }
      case "selectTab":
        options.onSelectTab(toShellTarget(event.payload));
        return;
      case "closeRequested":
        options.onCloseRequested();
        return;
    }
  }

  function onTransient(event: TransientStreamEvent) {
    useRuntimeEventStore.getState().pushTransientEvent(event);
    // Country flags use persisted query data, never a long-lived event
    // overlay: an old event must not resurrect a flag after a profile edit.
    // Only the node list carries flags (delays come from the live overlay),
    // and a full refetch of it per result was the dominant cost of a long run,
    // so results coalesce into one list refresh per COUNTRY_REFRESH_MS. The
    // run's closing invalidation covers the tail.
    if (event.kind === "speedtestResults"
      && event.payload.some((result) => !["waiting", "testing"].includes(result.outcome))
      && countryRefresh === undefined) {
      countryRefresh = setTimeout(() => {
        countryRefresh = undefined;
        void options.queryClient.invalidateQueries({ queryKey: queryKeys.profileList });
      }, COUNTRY_REFRESH_MS);
    }
  }

  return {
    onAppEvent,
    onInvalidate,
    onTransient,
    dispose: () => {
      clearTimeout(countryRefresh);
      countryRefresh = undefined;
    },
  };
}

function toShellTarget(tab: ShellTabTarget): ShellTarget {
  switch (tab) {
    case "logs":
      return { tab: "logs" };
    case "profiles":
      return { tab: "profiles" };
    case "proxyConnections":
      return { tab: "connections", view: "connections" };
  }
}

function reportEventRouterError(context: string, error: unknown) {
  if (typeof console === "undefined") {
    return;
  }

  console.error(`[event-router] ${context}: ${getErrorMessage(error)}`);
}
