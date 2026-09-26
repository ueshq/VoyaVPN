import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { HeroUINativeConfig } from "heroui-native/provider";
import { HeroUINativeProvider } from "heroui-native/provider";
import { render } from "@testing-library/react-native";
import type { ReactElement, ReactNode } from "react";

/**
 * The providers every screen test mounts under.
 *
 * `HeroUINativeProvider` is not decoration: it renders the portal host that
 * HeroUI's overlays, such as the node list's sort menu, mount into, so a test
 * without it never sees their content. Toasts stay off and animations are
 * disabled so a press is synchronous.
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

let renderedClient: QueryClient | null = null;

/**
 * Renders a screen under the providers with a fresh query client. The client
 * keeps refetch and garbage-collection timers Jest would wait on, so
 * `setup.ts` clears it after every test.
 */
export async function renderScreen(ui: ReactElement) {
  const queryClient = makeTestQueryClient();
  renderedClient = queryClient;
  const wrapper = ({ children }: { children: ReactNode }) => (
    <TestProviders queryClient={queryClient}>{children}</TestProviders>
  );

  return { queryClient, ...(await render(ui, { wrapper })) };
}

export function clearRenderedClient() {
  renderedClient?.clear();
  renderedClient = null;
}
