import { createBottomTabNavigator } from "@react-navigation/bottom-tabs";
import { DarkTheme, DefaultTheme, NavigationContainer } from "@react-navigation/native";
import { QueryClientProvider } from "@tanstack/react-query";
import { useI18n } from "@voya/i18n/use-i18n";
import { Suspense, use } from "react";
import { Text, View } from "react-native";
import { SafeAreaProvider } from "react-native-safe-area-context";

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
 * A placeholder for a tab that has no screen yet.
 *
 * Every tab is wired from the start so the navigator, the shared translations
 * and the `ShellTab` union are exercised end to end; the screens replace these
 * one at a time.
 */
function PendingScreen({ tab }: { tab: ShellTab }) {
  const { t } = useI18n();

  return (
    <View className="flex-1 items-center justify-center bg-canvas">
      <Text className="text-section text-foreground">{t(SHELL_TABS[tab].titleKey)}</Text>
    </View>
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

  return (
    <NavigationContainer ref={navigationRef} theme={scheme === "dark" ? DarkTheme : DefaultTheme}>
      <EventBridge />
      <Tab.Navigator>
        {(Object.keys(SHELL_TABS) as ShellTab[]).map((tab) => (
          <Tab.Screen key={tab} name={tab} options={{ title: t(SHELL_TABS[tab].titleKey) }}>
            {() => <PendingScreen tab={tab} />}
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
