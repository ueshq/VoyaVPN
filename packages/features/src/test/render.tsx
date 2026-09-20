import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, type RenderHookResult } from "@testing-library/react";

type WithQueryClient = { queryClient: QueryClient };

/**
 * A query client for one test: a failed query fails at once instead of
 * retrying. Pass `gcTime: 0` to drop data as soon as nothing observes it.
 */
export function createTestQueryClient(queries: { gcTime?: number } = {}): QueryClient {
  return new QueryClient({ defaultOptions: { queries: { ...queries, retry: false } } });
}

/** Renders a hook inside a query client. */
export function renderHookWithQuery<Result>(
  hook: () => Result,
  { queryClient = createTestQueryClient() }: Partial<WithQueryClient> = {},
): RenderHookResult<Result, unknown> & WithQueryClient {
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return Object.assign(renderHook(hook, { wrapper }), { queryClient });
}
