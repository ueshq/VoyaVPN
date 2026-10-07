import { ConnectionLaunch } from "~/features/connection/connection-launch";
import { NodePickerScreen } from "~/features/profiles/node-picker-screen";
import { SessionDetailsScreen } from "~/features/home/session-details-screen";
import { ImportScreen } from "~/features/profiles/import-screen";
import { ProfileEditorScreen } from "~/features/profiles/profile-editor-screen";
import { SubscriptionsScreen, SubscriptionScreen } from "~/features/profiles/subscriptions-screen";
import { ConnectionDetailsScreen } from "~/features/proxy/connection-details-screen";
import { GeneralScreen, MaintenanceScreen } from "~/features/settings/preferences-screens";
import { DnsScreen } from "~/features/settings/dns-screen";
import { AboutScreen } from "~/features/settings/about-screen";
import { StartupFailureScreen } from "~/features/settings/startup-failure-screen";
import { isPrivacyNoticeAccepted } from "~/features/settings/privacy-notice";
import { PrivacyNoticeScreen } from "~/features/settings/privacy-notice-screen";
import { LogsScreen } from "~/features/settings/logs-screen";
import { RuleDetailsScreen } from "~/features/routing/rule-details-screen";
import { DarkTheme, DefaultTheme, NavigationContainer, useIsFocused } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { createNativeBottomTabNavigator } from "@bottom-tabs/react-navigation";
import { QueryClientProvider } from "@tanstack/react-query";
import { useI18n } from "@voya/i18n/use-i18n";
import type { AppError } from "@voya/contracts";
import { appErrorOfKind } from "@voya/client/errors";
import { queries } from "@voya/client/queries";
import type { HeroUINativeConfig } from "heroui-native/provider";
import { HeroUINativeProvider } from "heroui-native/provider";
import { type ComponentType, Suspense, useCallback, use, useEffect, useState } from "react";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { Platform, View } from "react-native";
import { SafeAreaProvider, useSafeAreaInsets } from "react-native-safe-area-context";

import { SafeAreaView as NativeSafeAreaView } from "react-native-screens/experimental";
import { NativeTabSafeAreaContext } from "~/components/use-screen-insets";
import { useClassColor, useToneColor } from "~/components/tone";
import { StackBackButton } from "~/components/stack-back-button";

import { ActivityScreen } from "~/features/proxy/activity-screen";
import { EventBridge } from "~/ipc/event-bridge";
import { voyaTransport } from "~/ipc/platform";
import { localeReady } from "~/native/platform-boot";

import { type RootRoutes, navigationRef } from "./navigation";
import { NoticeHost } from "./notice-host";
import { guarded } from "./guarded";
import { ScreenErrorBoundary } from "./screen-error-boundary";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { createAppQueryClient } from "@voya/client/query-client";
import type { ShellTab } from "./tabs";
import { SHELL_TABS } from "./tabs";
import { ScreenActiveContext } from "@voya/features/shell/screen-active";
import { useRuntimeStatusSeed } from "@voya/features/shell/use-runtime-status-seed";
import { useTheme } from "./use-theme";

const Tab = createNativeBottomTabNavigator<Record<ShellTab, undefined>>();
const Stack = createNativeStackNavigator<RootRoutes>();
const queryClient = createAppQueryClient();

/**
 * Each tab's screen, told whether it is the one in front.
 *
 * The tabs stay mounted behind the one showing, and behind any page pushed
 * over them, so a screen that polls or ticks has to know when nobody is
 * looking. Built once: a component made per render would remount its tab.
 */
function focusAware(Screen: ComponentType): ComponentType {
  const GuardedScreen = guarded(Screen);
  return function TabScreen() {
    return (
      // Android lays scenes above its bar, which also consumes the system bottom inset.
      // iOS scenes extend behind the native bar, so its safe area owns that edge.
      <NativeSafeAreaView edges={{ top: true, bottom: Platform.OS === "ios", left: true, right: true }}>
        <NativeTabSafeAreaContext value={true}>
          <ScreenActiveContext value={useIsFocused()}>
            <GuardedScreen />
          </ScreenActiveContext>
        </NativeTabSafeAreaContext>
      </NativeSafeAreaView>
    );
  };
}

const TAB_SCREENS: Record<ShellTab, ComponentType> = {
  home: focusAware(SHELL_TABS.home.component),
  profiles: focusAware(SHELL_TABS.profiles.component),
  rules: focusAware(SHELL_TABS.rules.component),
  settings: focusAware(SHELL_TABS.settings.component),
};

/**
 * The pages pushed over the tabs, each inside a boundary of its own: one that
 * throws while rendering shows a retry in its place, and back still works.
 * Wrapped once, here — a component made per render would remount its page.
 */
const PAGES = {
  nodePicker: guarded(NodePickerScreen),
  sessionDetails: guarded(SessionDetailsScreen),
  about: guarded(AboutScreen),
  activity: guarded(ActivityScreen),
  connectionDetails: guarded(ConnectionDetailsScreen),
  dns: guarded(DnsScreen),
  editProfile: guarded(ProfileEditorScreen),
  general: guarded(GeneralScreen),
  import: guarded(ImportScreen),
  logs: guarded(LogsScreen),
  maintenance: guarded(MaintenanceScreen),
  ruleDetails: guarded(RuleDetailsScreen),
  subscription: guarded(SubscriptionScreen),
  subscriptions: guarded(SubscriptionsScreen),
};

function MainTabs() {
  const { t } = useI18n();
  const activeColor = useToneColor("brand");
  const inactiveColor = useToneColor("neutral");
  const surfaceColor = useClassColor("bg-surface", "backgroundColor");
  const indicatorColor = useClassColor("bg-brand-tint", "backgroundColor");
  return (
    <Tab.Navigator
      labeled
      tabBarActiveTintColor={activeColor}
      // Material caches its native theme while this navigator is covered by a
      // settings page. Keep its colors in sync with the app's live preference.
      tabBarInactiveTintColor={Platform.OS === "android" ? inactiveColor : undefined}
      tabBarStyle={Platform.OS === "android" ? { backgroundColor: surfaceColor } : undefined}
      activeIndicatorColor={Platform.OS === "android" ? indicatorColor : undefined}
      minimizeBehavior="never"
    >
      {(Object.keys(SHELL_TABS) as ShellTab[]).map((tab) => {
        const { icon, titleKey } = SHELL_TABS[tab];
        return (
          <Tab.Screen
            key={tab}
            name={tab}
            component={TAB_SCREENS[tab]}
            options={{
              // UIKit and Android derive the accessible tab name from its title.
              title: t(titleKey),
              tabBarIcon: () => icon,
            }}
          />
        );
      })}
    </Tab.Navigator>
  );
}

/** The app itself, once `StartupGate` has let it through. */
function Shell() {
  const { t } = useI18n();
  const scheme = useTheme();
  const insets = useSafeAreaInsets();
  const baseTheme = scheme === "dark" ? DarkTheme : DefaultTheme;
  const canvas = useClassColor("bg-canvas", "backgroundColor");
  const navigationTheme = {
    ...baseTheme,
    colors: { ...baseTheme.colors, background: canvas ?? baseTheme.colors.background },
  };
  useRuntimeStatusSeed();
  const [navigationReady, setNavigationReady] = useState(false);
  const noticeAccepted = usePreferencesStore((state) => isPrivacyNoticeAccepted(state.privacyNoticeVersion));

  // Before first use the app shows what data it handles (App Store Guideline
  // 5.4). The notice replaces navigation rather than covering it, so nothing
  // behind it can be reached, and the event bridge mounts only afterwards.
  if (!noticeAccepted) return <PrivacyNoticeScreen />;

  return (
    <NavigationContainer ref={navigationRef} theme={navigationTheme} onReady={() => setNavigationReady(true)}>
      {navigationReady ? <ConnectionLaunch /> : null}
      <EventBridge />
      <NoticeHost />
      {/* Stack pages cover the native tabs and show the navigation bar with
          their translated title, and a page that needs a scrolling
          large title draws one itself. The back chevron is `StackBackButton`:
          the native one is invisible to React Native accessibility, so the
          header draws a labelled button instead. */}
      <Stack.Navigator
        screenOptions={({ navigation }) => ({
          headerBackVisible: false,
          headerLeft: ({ canGoBack }) => (canGoBack ? <StackBackButton onPress={() => navigation.goBack()} /> : null),
          headerShadowVisible: false,
          statusBarStyle: scheme === "dark" ? "light" : "dark",
        })}
      >
        <Stack.Screen name="main" component={MainTabs} options={{ headerShown: false, title: "VoyaVPN" }} />
        <Stack.Screen
          name="nodePicker"
          component={PAGES.nodePicker}
          options={{ title: t("home.chooseNode"), presentation: "modal" }}
        />
        <Stack.Screen
          name="sessionDetails"
          component={PAGES.sessionDetails}
          options={{ title: t("activity.connectionDetails") }}
        />
        <Stack.Screen name="activity" component={PAGES.activity} options={{ title: t("tabs.connections") }} />
        <Stack.Screen
          name="connectionDetails"
          component={PAGES.connectionDetails}
          options={{ title: t("activity.connectionDetails") }}
        />
        <Stack.Screen name="import" component={PAGES.import} options={{ title: t("mobile.add") }} />
        <Stack.Screen
          name="subscriptions"
          component={PAGES.subscriptions}
          options={{ title: t("mobile.subscriptions") }}
        />
        <Stack.Screen
          name="subscription"
          component={PAGES.subscription}
          options={{ title: t("mobile.subscription") }}
        />
        <Stack.Screen name="editProfile" component={PAGES.editProfile} options={{ title: t("mobile.editNode") }} />
        <Stack.Screen name="general" component={PAGES.general} options={{ title: t("daily.appearance") }} />
        <Stack.Screen name="dns" component={PAGES.dns} options={{ title: t("mobile.connection") }} />
        <Stack.Screen name="maintenance" component={PAGES.maintenance} options={{ title: t("daily.diagnostics") }} />
        <Stack.Screen name="logs" component={PAGES.logs} options={{ title: t("tabs.logs") }} />
        <Stack.Screen name="about" component={PAGES.about} options={{ title: t("daily.about") }} />
        <Stack.Screen name="ruleDetails" component={PAGES.ruleDetails} options={{ title: t("mobile.ruleDetails") }} />
      </Stack.Navigator>
      {/* Without a navigation bar nothing covers the status bar, so content
          scrolled up would run under the clock. A band of canvas does what
          the bar's background did; it is invisible until something is under
          it, and sheets still cover it because they mount above the shell. */}
      <View
        pointerEvents="none"
        accessible={false}
        className="absolute inset-x-0 top-0 bg-canvas"
        style={{ height: insets.top }}
      />
    </NavigationContainer>
  );
}

/**
 * HeroUI's toast overlay is not mounted: a notice reaches the user as a
 * system alert (`NoticeHost`), not a toast. The styling primer HeroUI prints
 * on every development launch is switched off.
 */
const HEROUI_CONFIG: HeroUINativeConfig = {
  devInfo: { stylingPrinciples: false },
  toast: false,
};

/**
 * The provider stack.
 *
 * `GestureHandlerRootView` is outermost because a gesture outside it is never
 * recognised, and HeroUI's press feedback is one.
 *
 * `HeroUINativeProvider` sits outside `Suspense` because it renders the portal
 * host that HeroUI's overlays mount into: inside, every suspension of `Shell`
 * would unmount the host along with whatever was open in it. It sits inside
 * `QueryClientProvider` because portal content renders at the host, not where
 * it was declared — so an overlay can read the query client, but not
 * navigation, which lives in `Shell`. The node sort and actions sheets are
 * React Native `Modal`s, not portals, so none of this applies to them.
 */
export function App() {
  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <SafeAreaProvider>
        <QueryClientProvider client={queryClient}>
          <HeroUINativeProvider config={HEROUI_CONFIG}>
            <Suspense fallback={null}>
              <StartupGate />
            </Suspense>
          </HeroUINativeProvider>
        </QueryClientProvider>
      </SafeAreaProvider>
    </GestureHandlerRootView>
  );
}

/**
 * Watches for the one failure nothing behind it survives: a database the
 * backend refuses to open. While the check runs nothing renders (the shell's
 * own queries would all fail against the same dead backend a moment later);
 * a rejected database takes over with the reset flow, and a successful reset
 * remounts the shell with fresh caches against the new database.
 *
 * It also suspends until the startup locale's resources are in place. The
 * desktop entry delays `render()` on the same promise; React Native mounts the
 * registered component immediately instead, so the gate has to live here.
 * Without it a non-English launch paints English first and then swaps.
 */
function StartupGate() {
  use(localeReady);
  const [attempt, setAttempt] = useState(0);
  const [resetting, setResetting] = useState(false);
  const [resetFailure, setResetFailure] = useState<unknown>(null);
  const startup = useBackendStartup(attempt);

  const reset = useCallback(() => {
    setResetting(true);
    setResetFailure(null);
    voyaTransport()
      .resetApplicationData()
      .then(() => {
        queryClient.clear();
        setAttempt((attempt) => attempt + 1);
        setResetting(false);
      })
      .catch((failure: unknown) => {
        // The move failed; the database is untouched, so the user can try
        // again — but has to be told it failed, or a button that merely came
        // back to life reads as a reset that did nothing.
        setResetFailure(failure);
        setResetting(false);
      });
  }, []);

  if (startup.status === "failed") {
    return <StartupFailureScreen error={startup.error} busy={resetting} resetFailure={resetFailure} onReset={reset} />;
  }
  if (startup.status === "checking") return null;

  // The key makes a post-reset remount rebuild every query cache and screen
  // rather than reuse the ones that watched the failure.
  // The boundary is the last line of defence: each screen has its own, and
  // this one covers what is around them — navigation itself, the bridges.
  return (
    <ScreenErrorBoundary>
      <Shell key={attempt} />
    </ScreenErrorBoundary>
  );
}

type StartupSettled = { status: "ok" } | { status: "failed"; error: AppError };
type StartupWatch = { status: "checking" } | StartupSettled;

function useBackendStartup(attempt: number): StartupWatch {
  // The result remembers which attempt it answers; a new attempt is "checking"
  // by derivation rather than by a state write inside the effect.
  const [result, setResult] = useState<{ attempt: number; value: StartupSettled } | null>(null);

  useEffect(() => {
    let cancelled = false;
    // The probe is the settings read every screen needs anyway, so its answer
    // goes into the cache they read from rather than being fetched twice. A
    // retry after a reset must ask the new database, not reuse the answer.
    queryClient
      .fetchQuery({ ...queries.appSettings, staleTime: 0 })
      .then(() => {
        if (!cancelled) setResult({ attempt, value: { status: "ok" } });
      })
      .catch((error: unknown) => {
        if (cancelled) return;
        const appError = appErrorOfKind(error, "database");
        if (appError && (appError.kind.code === "schemaUnsupported" || appError.kind.code === "corrupt")) {
          setResult({ attempt, value: { status: "failed", error: appError } });
        } else {
          // Any other failure is a normal command error: the shell's screens
          // surface it where it happened, so startup must not swallow it.
          setResult({ attempt, value: { status: "ok" } });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  return result !== null && result.attempt === attempt ? result.value : { status: "checking" };
}
