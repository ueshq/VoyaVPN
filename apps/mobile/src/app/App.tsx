import { createBottomTabNavigator } from "@react-navigation/bottom-tabs";
import { DarkTheme, DefaultTheme, NavigationContainer } from "@react-navigation/native";
import { QueryClientProvider } from "@tanstack/react-query";
import { useI18n } from "@voya/i18n/use-i18n";
import { Suspense, use } from "react";
import { SafeAreaProvider } from "react-native-safe-area-context";

import { HomeScreen } from "~/features/home/home-screen";
import { NodesScreen } from "~/features/profiles/nodes-screen";
import { ActivityScreen } from "~/features/proxy/activity-screen";
import { RulesScreen } from "~/features/routing/rules-screen";
import { SettingsScreen } from "~/features/settings/settings-screen";
import { EventBridge } from "~/ipc/event-bridge";
import { localeReady } from "~/native/platform-boot";

import { navigationRef } from "./navigation";
import { createMobileQueryClient } from "./query-client";
import type { ShellTab } from "./tabs";
import { SHELL_TABS } from "./tabs";
import { useTheme } from "./use-theme";

const Tab = createBottomTabNavigator();
const queryClient = createMobileQueryClient();

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

export function App() {
  return (
    <SafeAreaProvider>
      <QueryClientProvider client={queryClient}>
        <Suspense fallback={null}>
          <Shell />
        </Suspense>
      </QueryClientProvider>
    </SafeAreaProvider>
  );
}
