import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { HeroUINativeConfig } from "heroui-native/provider";
import { HeroUINativeProvider } from "heroui-native/provider";
import type { ReactNode } from "react";

/**
 * The providers every screen test mounts under.
 *
 * `HeroUINativeProvider` is not decoration: it renders the portal host that
 * sheets mount into, so a test without it never sees panel content. Toasts
 * stay off and animations are disabled so a press is synchronous.
 */
const Stack = createNativeStackNavigator();

const TEST_HEROUI_CONFIG: HeroUINativeConfig = {
  animation: "disable-all",
  devInfo: { stylingPrinciples: false },
  toast: false,
};

/**
 * A fresh client per test. Lives beside the wrapper so a suite can clear its
 * timers in `afterEach`; fast refresh never sees this test-only module.
 */
// eslint-disable-next-line react-refresh/only-export-components
export function makeTestQueryClient() {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } });
}

/**
 * Pass the test's own query client so `afterEach` can clear its timers —
 * `render`'s `wrapper` closes over it.
 */
export function TestProviders({
  children,
  queryClient,
}: {
  children: ReactNode;
  queryClient: QueryClient;
}) {
  return (
    <QueryClientProvider client={queryClient}>
      <HeroUINativeProvider config={TEST_HEROUI_CONFIG}><NavigationContainer><Stack.Navigator screenOptions={{ headerShown: false, animation: "none" }}><Stack.Screen name="test">{() => children}</Stack.Screen></Stack.Navigator></NavigationContainer></HeroUINativeProvider>
    </QueryClientProvider>
  );
}
