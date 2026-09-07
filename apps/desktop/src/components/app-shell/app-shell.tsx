import { lazy, Suspense, useEffect, useMemo, useRef, type MutableRefObject } from "react";

import { AppSidebar, SHELL_PANEL_ID } from "@/components/app-shell/app-sidebar";
import { AppErrorBoundary } from "@/components/app-shell/error-boundary";
import { ModalHost } from "@/components/app-shell/modal-host";
import { TitleBar } from "@/components/app-shell/title-bar";
import { Toaster } from "@/components/app-shell/toaster";
import { useAcrylicWindow } from "@/components/app-shell/use-acrylic-window";
import { useRuntimeStatusSeed } from "@/components/app-shell/use-runtime-status-seed";
import { useWindowChrome } from "@/components/app-shell/use-window-chrome";
import { useI18n } from "@voya/i18n/use-i18n";
import { proxyStartMonitor, proxyStopMonitor, useRuntimeEventStore } from "@/ipc";
import type { ProxyMonitorStatus } from "@/ipc/bindings";
import { type ShellTab, useShellStore } from "@/stores/shell-store";
import { useToastStore } from "@/stores/toast-store";

const HomeScreen = lazy(() =>
  import("@/features/home/home-screen").then(({ HomeScreen }) => ({ default: HomeScreen })),
);
const ProfilesScreen = lazy(() =>
  import("@/features/profiles/server-table").then(({ ProfilesScreen }) => ({
    default: ProfilesScreen,
  })),
);
const RulesScreen = lazy(() =>
  import("@/features/routing/routing-screen").then(({ RoutingScreen }) => ({
    default: RoutingScreen,
  })),
);
const ProxyGroupsScreen = lazy(() =>
  import("@/features/proxy/proxy-groups-screen").then(({ ProxyGroupsScreen }) => ({
    default: ProxyGroupsScreen,
  })),
);
const ConnectionsScreen = lazy(() =>
  import("@/features/proxy/connections-screen").then(({ ConnectionsScreen }) => ({
    default: ConnectionsScreen,
  })),
);
const SettingsScreen = lazy(() =>
  import("@/features/settings/settings-screen").then(({ SettingsScreen }) => ({
    default: SettingsScreen,
  })),
);

// Render only the active screen. Replaces the Radix `Tabs`/`TabsContent` fan-out
// (which already unmounted inactive panels) so the grid shell can drop the tab
// primitive while keeping the exact "one mounted screen at a time" behaviour the
// proxy-monitor lifecycle and query work rely on.
function renderActiveScreen(tab: ShellTab) {
  switch (tab) {
    case "home":
      return <HomeScreen />;
    case "profiles":
      return <ProfilesScreen />;
    case "rules":
      return <RulesScreen />;
    case "proxies":
      return <ProxyGroupsScreen />;
    case "connections":
      return <ConnectionsScreen />;
    case "settings":
      return <SettingsScreen />;
    default:
      return null;
  }
}

export function AppShell() {
  const { direction } = useI18n();
  const activeTab = useShellStore((state) => state.activeTab);
  const { titleBarLayout } = useWindowChrome();

  useProxyMonitorLifecycle(activeTab);
  useRuntimeStatusSeed();
  // Windows borderless chrome is the only Acrylic target; the hook no-ops elsewhere.
  useAcrylicWindow(titleBarLayout === "windows");

  return (
    <main className="bg-background text-foreground" dir={direction}>
      <div className="grid h-screen min-h-[34rem] grid-cols-[auto_1fr] grid-rows-[auto_1fr] overflow-hidden">
        {/* Titlebar row: the Windows build draws its own borderless title bar
            (it spans both columns); every other platform keeps its native frame
            and leaves this structural row empty (collapsing to zero height). */}
        {titleBarLayout === "windows" ? (
          <TitleBar />
        ) : (
          <div className="col-span-2" data-slot="titlebar-placeholder" />
        )}

        <AppSidebar />

        <div
          aria-labelledby={`shell-tab-${activeTab}`}
          className="min-h-0 min-w-0 overflow-hidden bg-background outline-none"
          id={SHELL_PANEL_ID}
          role="tabpanel"
          tabIndex={0}
        >
          {/* Keyed on the active tab so a crashed screen recovers by navigating
              away, and so the sidebar/footer survive a screen render error or a
              rejected lazy chunk instead of the whole root unmounting. */}
          <AppErrorBoundary resetKey={activeTab}>
            <Suspense fallback={<ScreenFallback />}>{renderActiveScreen(activeTab)}</Suspense>
          </AppErrorBoundary>
        </div>
      </div>

      <ModalHost />
      <Toaster />
    </main>
  );
}

function ScreenFallback() {
  const { t } = useI18n();
  return <div className="h-full animate-pulse bg-surface-raised/40" aria-label={t("status.loadingScreen")} />;
}

function useProxyMonitorLifecycle(activeTab: ShellTab) {
  const { t } = useI18n();
  const pushToast = useToastStore((state) => state.pushToast);
  const messages = useMemo<ProxyMonitorMessages>(
    () => ({
      startFallback: t("status.proxyMonitorStartFailed"),
      stopFallback: t("status.proxyMonitorStopFailed"),
      title: t("status.proxyRuntime"),
    }),
    [t],
  );
  const startTimerRef = useRef<number | null>(null);
  const stopTimerRef = useRef<number | null>(null);
  const runningRef = useRef(false);
  const startingRef = useRef(false);
  const stoppingRef = useRef(false);
  const wantsMonitorRef = useRef(false);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return undefined;
    }

    wantsMonitorRef.current = isProxyTab(activeTab);
    clearTimer(startTimerRef);
    clearTimer(stopTimerRef);

    if (wantsMonitorRef.current) {
      if (!runningRef.current && !startingRef.current && !stoppingRef.current) {
        scheduleProxyMonitorStart({
          pushToast,
          messages,
          runningRef,
          startingRef,
          startTimerRef,
          stoppingRef,
          stopTimerRef,
          wantsMonitorRef,
        });
      }

      return undefined;
    }

    if (runningRef.current || startingRef.current || stoppingRef.current) {
      scheduleProxyMonitorStop({
        pushToast,
        messages,
        runningRef,
        startingRef,
        startTimerRef,
        stoppingRef,
        stopTimerRef,
        wantsMonitorRef,
      });
    }

    return undefined;
  }, [activeTab, messages, pushToast]);

  useEffect(
    () => () => {
      clearTimer(startTimerRef);
      clearTimer(stopTimerRef);
      wantsMonitorRef.current = false;
      if (runningRef.current) {
        void proxyStopMonitor().catch((error: unknown) => {
          console.error("[proxy-monitor] failed to stop during cleanup", error);
        });
      }
    },
    [],
  );
}

type PushToast = ReturnType<typeof useToastStore.getState>["pushToast"];

type ProxyMonitorMessages = {
  startFallback: string;
  stopFallback: string;
  title: string;
};

type ProxyMonitorLifecycleRefs = {
  runningRef: MutableRefObject<boolean>;
  startingRef: MutableRefObject<boolean>;
  startTimerRef: MutableRefObject<number | null>;
  stoppingRef: MutableRefObject<boolean>;
  stopTimerRef: MutableRefObject<number | null>;
  wantsMonitorRef: MutableRefObject<boolean>;
};

function scheduleProxyMonitorStart({
  messages,
  pushToast,
  runningRef,
  startingRef,
  startTimerRef,
  stoppingRef,
  stopTimerRef,
  wantsMonitorRef,
}: ProxyMonitorLifecycleRefs & { messages: ProxyMonitorMessages; pushToast: PushToast }) {
  clearTimer(startTimerRef);
  startTimerRef.current = window.setTimeout(() => {
    startTimerRef.current = null;
    if (!wantsMonitorRef.current || runningRef.current || startingRef.current || stoppingRef.current) {
      return;
    }

    startingRef.current = true;
    useRuntimeEventStore.getState().setProxyMonitorStarting();
    void proxyStartMonitor()
      .then((status) => {
        applyProxyMonitorStatus(status, runningRef);
        if (!wantsMonitorRef.current && status.running) {
          scheduleProxyMonitorStop({
            pushToast,
            messages,
            runningRef,
            startingRef,
            startTimerRef,
            stoppingRef,
            stopTimerRef,
            wantsMonitorRef,
          });
        }
      })
      .catch((error) => {
        const message = proxyMonitorErrorMessage(error, messages.startFallback);

        runningRef.current = false;
        useRuntimeEventStore.getState().setProxyMonitorFailed(message);
        pushToast({ description: message, severity: "error", title: messages.title });
      })
      .finally(() => {
        startingRef.current = false;
      });
  }, 100);
}

function scheduleProxyMonitorStop({
  messages,
  pushToast,
  runningRef,
  startingRef,
  startTimerRef,
  stoppingRef,
  stopTimerRef,
  wantsMonitorRef,
}: ProxyMonitorLifecycleRefs & { messages: ProxyMonitorMessages; pushToast: PushToast }) {
  clearTimer(stopTimerRef);
  stopTimerRef.current = window.setTimeout(() => {
    stopTimerRef.current = null;
    if (!runningRef.current && !startingRef.current && !stoppingRef.current) {
      return;
    }

    stoppingRef.current = true;
    void proxyStopMonitor()
      .then((status) => {
        applyProxyMonitorStatus(status, runningRef);
      })
      .catch((error) => {
        const message = proxyMonitorErrorMessage(error, messages.stopFallback);

        runningRef.current = false;
        useRuntimeEventStore.getState().setProxyMonitorFailed(message);
        pushToast({ description: message, severity: "error", title: messages.title });
      })
      .finally(() => {
        stoppingRef.current = false;
        if (wantsMonitorRef.current && !runningRef.current && !startingRef.current) {
          scheduleProxyMonitorStart({
            pushToast,
            messages,
            runningRef,
            startingRef,
            startTimerRef,
            stoppingRef,
            stopTimerRef,
            wantsMonitorRef,
          });
        }
      });
  }, 2_000);
}

function applyProxyMonitorStatus(status: ProxyMonitorStatus, runningRef: MutableRefObject<boolean>) {
  runningRef.current = status.running;
  useRuntimeEventStore.getState().setProxyMonitorStatus(status);
}

function proxyMonitorErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === "string" && error) {
    return error;
  }

  return fallback;
}

function clearTimer(timerRef: MutableRefObject<number | null>) {
  if (timerRef.current !== null) {
    window.clearTimeout(timerRef.current);
    timerRef.current = null;
  }
}

function isProxyTab(tab: ShellTab) {
  return tab === "proxies" || tab === "connections";
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
