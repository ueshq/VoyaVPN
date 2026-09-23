import type { ReactElement, ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  render,
  renderHook,
  type RenderHookResult,
  type RenderResult,
} from "@testing-library/react";

type WithQueryClient = { queryClient: QueryClient };

/**
 * A query client for one test: a failed query fails at once instead of
 * retrying. Pass `gcTime: 0` to drop data as soon as nothing observes it.
 */
export function createTestQueryClient(queries: { gcTime?: number } = {}): QueryClient {
  return new QueryClient({ defaultOptions: { queries: { ...queries, retry: false } } });
}

function withQueryClient(queryClient: QueryClient) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

/** Renders `ui` inside a query client; `rerender` keeps the same client. */
export function renderWithQuery(
  ui: ReactElement,
  { queryClient = createTestQueryClient() }: Partial<WithQueryClient> = {},
): RenderResult & WithQueryClient {
  return Object.assign(render(ui, { wrapper: withQueryClient(queryClient) }), { queryClient });
}

/** Renders a hook inside a query client. */
export function renderHookWithQuery<Result>(
  hook: () => Result,
  { queryClient = createTestQueryClient() }: Partial<WithQueryClient> = {},
): RenderHookResult<Result, unknown> & WithQueryClient {
  return Object.assign(renderHook(hook, { wrapper: withQueryClient(queryClient) }), { queryClient });
}
