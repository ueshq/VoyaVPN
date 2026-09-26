import { ImportScreen } from "~/features/profiles/import-screen";
import { SubscriptionsScreen, SubscriptionScreen } from "~/features/profiles/subscriptions-screen";
import { ConnectionDetailsScreen } from "~/features/proxy/connection-details-screen";
import { GeneralScreen, MaintenanceScreen } from "~/features/settings/preferences-screens";
import { DnsScreen } from "~/features/settings/dns-screen";
import { AboutScreen } from "~/features/settings/about-screen";
import { LogsScreen } from "~/features/settings/logs-screen";
import { RuleDetailsScreen } from "~/features/routing/rule-details-screen";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { createBottomTabNavigator } from "@react-navigation/bottom-tabs";
import { DarkTheme, DefaultTheme, NavigationContainer } from "@react-navigation/native";
import { QueryClientProvider } from "@tanstack/react-query";
import { useI18n } from "@voya/i18n/use-i18n";
import type { HeroUINativeConfig } from "heroui-native/provider";
import { HeroUINativeProvider } from "heroui-native/provider";
import { Suspense, use } from "react";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { View } from "react-native";
import { SafeAreaProvider, useSafeAreaInsets } from "react-native-safe-area-context";

import { FloatingTabBar } from "~/components/floating-tab-bar";

import { ActivityScreen } from "~/features/proxy/activity-screen";
import { EventBridge } from "~/ipc/event-bridge";
import { localeReady } from "~/native/platform-boot";

import { type RootRoutes, navigationRef } from "./navigation";
import { createAppQueryClient } from "@voya/client/query-client";
import type { ShellTab } from "./tabs";
import { SHELL_TABS } from "./tabs";
import { useRuntimeStatusSeed } from "@voya/features/shell/use-runtime-status-seed";
import { useTheme } from "./use-theme";

const Tab = createBottomTabNavigator();
const Stack = createNativeStackNavigator<RootRoutes>();
const queryClient = createAppQueryClient();

function MainTabs() {
  const { t } = useI18n();
  return (
    <Tab.Navigator tabBar={(props) => <FloatingTabBar {...props} />} screenOptions={{ headerShown: false }}>
      {(Object.keys(SHELL_TABS) as ShellTab[]).map((tab) => {
        const { component, icon: Icon, titleKey } = SHELL_TABS[tab];
        return (
          <Tab.Screen key={tab} name={tab} component={component} options={{
            // The floating bar derives the tab's label and VoiceOver name from this.
            title: t(titleKey),
            tabBarButtonTestID: `tab-${tab}`,
            tabBarIcon: ({ color, size }) => <Icon color={color} size={size} accessible={false} />,
          }} />
        );
      })}
    </Tab.Navigator>
  );
}

/**
 * The shell, suspended until the startup locale's resources are in place.
 *
 * The desktop entry delays `render()` on the same promise; React Native mounts
 * the registered component immediately instead, so the gate has to live here.
 * Without it a non-English launch paints English first and then swaps.
 */
function Shell() {
  use(localeReady);

  const { t } = useI18n();
  const scheme = useTheme();
  const insets = useSafeAreaInsets();
  const navigationTheme = scheme === "dark" ? DarkTheme : DefaultTheme;
  useRuntimeStatusSeed(["coreState"]);

  return (
    <NavigationContainer ref={navigationRef} theme={navigationTheme}>
      <EventBridge />
      {/* No navigation bar: each screen draws its own large title, which
          scrolls with its content. The tab bar floats over the content. */}
      <Stack.Navigator screenOptions={{ headerBackButtonDisplayMode: "minimal", headerShadowVisible: false, statusBarStyle: scheme === "dark" ? "light" : "dark" }}>
        <Stack.Screen name="main" component={MainTabs} options={{ headerShown: false, title: "VoyaVPN" }} />
        <Stack.Screen name="activity" component={ActivityScreen} options={{ title: t("tabs.connections") }} />
        <Stack.Screen name="connectionDetails" component={ConnectionDetailsScreen} options={{ title: t("mobile.connectionDetails") }} />
        <Stack.Screen name="import" component={ImportScreen} options={{ title: t("mobile.add") }} />
        <Stack.Screen name="subscriptions" component={SubscriptionsScreen} options={{ title: t("mobile.subscriptions") }} />
        <Stack.Screen name="subscription" component={SubscriptionScreen} options={{ title: t("mobile.subscription") }} />
        <Stack.Screen name="general" component={GeneralScreen} options={{ title: t("mobile.general") }} />
        <Stack.Screen name="dns" component={DnsScreen} options={{ title: t("mobile.connection") }} />
        <Stack.Screen name="maintenance" component={MaintenanceScreen} options={{ title: t("mobile.maintenance") }} />
        <Stack.Screen name="logs" component={LogsScreen} options={{ title: t("tabs.logs") }} />
        <Stack.Screen name="about" component={AboutScreen} options={{ title: t("mobile.about") }} />
        <Stack.Screen name="ruleDetails" component={RuleDetailsScreen} options={{ title: t("mobile.ruleDetails") }} />
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
 * Nothing shows a toast, so its overlay is not mounted, and the styling
 * primer HeroUI prints on every development launch is switched off.
 */
const HEROUI_CONFIG: HeroUINativeConfig = {
  devInfo: { stylingPrinciples: false },
  toast: false,
};

/**
 * The provider stack.
 *
 * `GestureHandlerRootView` is outermost because a gesture outside it is never
 * recognised, and HeroUI's sheet and press feedback are gestures.
 *
 * `HeroUINativeProvider` sits outside `Suspense` because it renders the portal
 * host that sheets mount into: inside, every suspension of `Shell` would
 * unmount the host along with whatever was open in it. It sits inside
 * `QueryClientProvider` because portal content renders at the host, not where
 * it was declared — so a sheet can read the query client, but not navigation,
 * which lives in `Shell`.
 */
export function App() {
  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <SafeAreaProvider>
        <QueryClientProvider client={queryClient}>
          <HeroUINativeProvider config={HEROUI_CONFIG}>
            <Suspense fallback={null}>
              <Shell />
            </Suspense>
          </HeroUINativeProvider>
        </QueryClientProvider>
      </SafeAreaProvider>
    </GestureHandlerRootView>
  );
}
