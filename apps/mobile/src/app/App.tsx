import { createBottomTabNavigator } from "@react-navigation/bottom-tabs";
import { DarkTheme, DefaultTheme, NavigationContainer } from "@react-navigation/native";
import { QueryClientProvider } from "@tanstack/react-query";
import { useI18n } from "@voya/i18n/use-i18n";
import type { HeroUINativeConfig } from "heroui-native/provider";
import { HeroUINativeProvider } from "heroui-native/provider";
import { Suspense, use } from "react";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { SafeAreaProvider } from "react-native-safe-area-context";

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
  useRuntimeStatusSeed(["coreState"]);

  return (
    <NavigationContainer ref={navigationRef} theme={scheme === "dark" ? DarkTheme : DefaultTheme}>
      <EventBridge />
      <Tab.Navigator>
        {(Object.keys(SHELL_TABS) as ShellTab[]).map((tab) => (
          <Tab.Screen key={tab} name={tab} options={{ title: t(SHELL_TABS[tab].titleKey) }}>
            {() => <TabScreen tab={tab} />}
          </Tab.Screen>
        ))}
      </Tab.Navigator>
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
