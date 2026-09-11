import { lazy, Suspense, useEffect, useMemo, useRef } from "react";

import { AppSidebar, SHELL_PANEL_ID } from "@/components/app-shell/app-sidebar";
import { AppErrorBoundary } from "@/components/app-shell/error-boundary";
import { ModalHost } from "@/components/app-shell/modal-host";
import {
  createProxyMonitorController,
  proxyMonitorErrorMessage,
  type ProxyMonitorController,
  type ProxyMonitorPhase,
} from "@/components/app-shell/proxy-monitor-controller";
import { TitleBar } from "@/components/app-shell/title-bar";
import { Toaster } from "@/components/app-shell/toaster";
import { useAcrylicWindow } from "@/components/app-shell/use-acrylic-window";
import { useRuntimeStatusSeed } from "@/components/app-shell/use-runtime-status-seed";
import { useWindowChrome } from "@/components/app-shell/use-window-chrome";
import { useI18n } from "@voya/i18n/use-i18n";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
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
    case "connections":
      return <ConnectionsScreen />;
    case "settings":
      return <SettingsScreen />;
    default:
      return null;
  }
}

export function AppShell() {
  const activeTab = useShellStore((state) => state.activeTab);
  const { titleBarLayout } = useWindowChrome();

  useProxyMonitorLifecycle(activeTab);
  useRuntimeStatusSeed();
  // Windows borderless chrome is the only Acrylic target; the hook no-ops elsewhere.
  useAcrylicWindow(titleBarLayout === "windows");

  return (
    <main className="bg-background text-foreground">
      <div className="app-shell" data-active-tab={activeTab} data-window-chrome={titleBarLayout}>
        <AppSidebar titleBarLayout={titleBarLayout} />

        <div className="shell-content-column">
          <TitleBar layout={titleBarLayout} />
          <div
            aria-labelledby={`shell-tab-${activeTab}`}
            className="shell-panel outline-none"
            id={SHELL_PANEL_ID}
            role="tabpanel"
            tabIndex={0}
          >
            {/* Keep the sidebar and window controls mounted when a screen fails. */}
            <AppErrorBoundary resetKey={activeTab}>
              <Suspense fallback={<ScreenFallback />}>{renderActiveScreen(activeTab)}</Suspense>
            </AppErrorBoundary>
          </div>
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

/**
 * Binds the proxy-monitor controller to the shell: the tab decides whether a
 * proxy-runtime surface is on screen, the controller owns everything else.
 */
function useProxyMonitorLifecycle(activeTab: ShellTab) {
  const coreConnected = useRuntimeEventStore((state) => state.coreState?.state === "connected");
  const { t } = useI18n();
  const pushToast = useToastStore((state) => state.pushToast);
  const messages = useMemo(
    () => ({
      start: t("status.proxyMonitorStartFailed"),
      stop: t("status.proxyMonitorStopFailed"),
      title: t("status.proxyRuntime"),
    }),
    [t],
  );
  // The controller is created once; this ref keeps its error path pointing at
  // the current locale's messages without recreating the state machine.
  const reportErrorRef = useRef<(error: unknown, phase: ProxyMonitorPhase) => void>(() => undefined);
  const controllerRef = useRef<ProxyMonitorController | null>(null);

  useEffect(() => {
    reportErrorRef.current = (error, phase) => {
      const message = proxyMonitorErrorMessage(error, phase === "start" ? messages.start : messages.stop);

      useRuntimeEventStore.getState().setProxyMonitorFailed(message);
      pushToast({ description: message, severity: "error", title: messages.title });
    };
  }, [messages, pushToast]);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return undefined;
    }

    const controller = createProxyMonitorController({
      onError: (error, phase) => reportErrorRef.current(error, phase),
    });
    controllerRef.current = controller;

    return () => {
      controllerRef.current = null;
      controller.dispose();
    };
  }, []);

  useEffect(() => {
    controllerRef.current?.setWanted(
      coreConnected && activeTab === "connections",
    );
  }, [activeTab, coreConnected]);
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
