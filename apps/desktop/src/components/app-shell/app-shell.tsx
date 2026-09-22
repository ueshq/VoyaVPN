import { lazy, Suspense, useEffect, useRef } from "react";

import { AppSidebar, SHELL_PANEL_ID } from "@/components/app-shell/app-sidebar";
import { CloseRequestDialog } from "@/components/app-shell/close-request-dialog";
import { AppErrorBoundary } from "@/components/app-shell/error-boundary";
import { ModalHost } from "@/components/app-shell/modal-host";
import {
  createProxyMonitorController,
  type ProxyMonitorController,
  type ProxyMonitorPhase,
} from "@/components/app-shell/proxy-monitor-controller";
import { TitleBar } from "@/components/app-shell/title-bar";
import { Toaster } from "@/components/app-shell/toaster";
import { useAcrylicWindow } from "@/components/app-shell/use-acrylic-window";
import { useRuntimeStatusSeed } from "@voya/features/shell/use-runtime-status-seed";
import { useShellShortcuts } from "@/components/app-shell/use-shell-shortcuts";
import { useWindowChrome } from "@/components/app-shell/use-window-chrome";
import { WindowChromeContext } from "@/components/app-shell/window-chrome-context";
import { useI18n } from "@voya/i18n/use-i18n";
import { Skeleton } from "@voya/ui/components/skeleton";
import { getErrorMessage } from "@voya/utils/error";
import { redactOperationalMessage } from "@voya/utils/operational-redaction";
import { useLatestRef } from "@voya/utils/use-latest-ref";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { isTauriRuntime } from "@/ipc/window";
import { useAppVisible } from "@voya/client/use-app-visible";
import { type ShellTab, useShellStore } from "@/stores/shell-store";
import { toastError } from "@voya/client/toast-store";

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
const SelfHostScreen = lazy(() =>
  import("@/features/self-host/self-host-screen").then(({ SelfHostScreen }) => ({
    default: SelfHostScreen,
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
    case "selfHost":
      return <SelfHostScreen />;
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
  useRuntimeStatusSeed(["coreState", "sysProxy", "tun"]);
  useShellShortcuts();
  // Windows borderless chrome is the only Acrylic target; the hook no-ops elsewhere.
  useAcrylicWindow(titleBarLayout === "windows");

  return (
    <main className="bg-surface-canvas text-foreground">
      <WindowChromeContext value={titleBarLayout}>
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
      </WindowChromeContext>

      <ModalHost />
      <CloseRequestDialog />
      <Toaster />
    </main>
  );
}

function ScreenFallback() {
  const { t } = useI18n();
  return (
    <div aria-label={t("status.loadingScreen")} className="grid h-full content-start gap-4 p-page" role="status">
      <Skeleton className="h-8 w-40" />
      <Skeleton className="h-40 w-full rounded-xl" />
      <Skeleton className="h-24 w-full rounded-xl" />
    </div>
  );
}

/**
 * Binds the proxy-monitor controller to the shell: the tab decides whether a
 * proxy-runtime surface is on screen, the controller owns everything else.
 * A window hidden into the tray shows nothing either, so the monitor also
 * stops then: the backend otherwise kept serializing the whole connection
 * table every second for a webview nobody was looking at.
 */
function useProxyMonitorLifecycle(activeTab: ShellTab) {
  const coreConnected = useRuntimeEventStore((state) => state.coreState?.state === "connected");
  const visible = useAppVisible();
  const { t } = useI18n();
  // The controller is created once; this ref keeps its error path pointing at
  // the current locale without recreating the state machine.
  const reportErrorRef = useLatestRef((error: unknown, phase: ProxyMonitorPhase) => {
    const fallback = t(phase === "start" ? "status.proxyMonitorStartFailed" : "status.proxyMonitorStopFailed");
    const message = redactOperationalMessage(getErrorMessage(error, fallback));

    useRuntimeEventStore.getState().setProxyMonitorFailed(message);
    toastError(t("status.proxyRuntime"), message);
  });
  const controllerRef = useRef<ProxyMonitorController | null>(null);

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
  }, [reportErrorRef]);

  useEffect(() => {
    controllerRef.current?.setWanted(
      coreConnected && activeTab === "connections" && visible,
    );
  }, [activeTab, coreConnected, visible]);
}
