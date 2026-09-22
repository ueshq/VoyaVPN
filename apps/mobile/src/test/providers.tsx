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
const TEST_HEROUI_CONFIG: HeroUINativeConfig = {
  animation: "disable-all",
  devInfo: { stylingPrinciples: false },
  toast: false,
};

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
      <HeroUINativeProvider config={TEST_HEROUI_CONFIG}>{children}</HeroUINativeProvider>
    </QueryClientProvider>
  );
}
