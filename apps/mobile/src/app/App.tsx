import { createBottomTabNavigator } from "@react-navigation/bottom-tabs";
import { DarkTheme, DefaultTheme, NavigationContainer } from "@react-navigation/native";
import { QueryClientProvider } from "@tanstack/react-query";
import { useI18n } from "@voya/i18n/use-i18n";
import type { HeroUINativeConfig } from "heroui-native/provider";
import { HeroUINativeProvider } from "heroui-native/provider";
import { Suspense, use } from "react";
import { Activity, House, Route, Server, Settings } from "lucide-react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { View } from "react-native";
import { SafeAreaProvider, useSafeAreaInsets } from "react-native-safe-area-context";

import { FloatingTabBar } from "~/components/floating-tab-bar";

import { HomeScreen } from "~/features/home/home-screen";
import { NodesScreen } from "~/features/profiles/nodes-screen";
import { ActivityScreen } from "~/features/proxy/activity-screen";
import { RulesScreen } from "~/features/routing/rules-screen";
import { SettingsScreen } from "~/features/settings/settings-screen";
import { EventBridge } from "~/ipc/event-bridge";
import { localeReady } from "~/native/platform-boot";

import { navigationRef } from "./navigation";
import { createAppQueryClient } from "@voya/client/query-client";
import type { ShellTab } from "./tabs";
import { SHELL_TABS } from "./tabs";
import { useRuntimeStatusSeed } from "@voya/features/shell/use-runtime-status-seed";
import { useTheme } from "./use-theme";

const Tab = createBottomTabNavigator();
const TAB_ICONS = { home: House, profiles: Server, rules: Route, connections: Activity, settings: Settings };
const queryClient = createAppQueryClient();

/**
 * The shell, suspended until the startup locale's resources are in place.
 *
 * The desktop entry delays `render()` on the same promise; React Native mounts
 * the registered component immediately instead, so the gate has to live here.
 * Without it a non-English launch paints English first and then swaps.
 */
/** The screen behind one tab. */
function TabScreen({ tab }: { tab: ShellTab }) {
  switch (tab) {
    case "home":
      return <HomeScreen />;
    case "profiles":
      return <NodesScreen />;
    case "rules":
      return <RulesScreen />;
    case "connections":
      return <ActivityScreen />;
    case "settings":
      return <SettingsScreen />;
  }
}

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
      <Tab.Navigator
        tabBar={(props) => <FloatingTabBar {...props} />}
        screenOptions={{ headerShown: false }}
      >
        {(Object.keys(SHELL_TABS) as ShellTab[]).map((tab) => (
          <Tab.Screen key={tab} name={tab} options={{
            title: t(SHELL_TABS[tab].titleKey),
            tabBarButtonTestID: `tab-${tab}`,
            tabBarLabel: tab === "connections" ? t("tabs.activityShort") : t(SHELL_TABS[tab].titleKey),
            tabBarAccessibilityLabel: t(SHELL_TABS[tab].titleKey),
            tabBarIcon: ({ color, size }) => {
              const Icon = TAB_ICONS[tab];
              return <Icon color={color} size={size} accessible={false} />;
            },
          }}>
            {() => <TabScreen tab={tab} />}
          </Tab.Screen>
        ))}
      </Tab.Navigator>
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
