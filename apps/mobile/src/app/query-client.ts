import { MutationCache, QueryClient, type Mutation } from "@tanstack/react-query";

import { toastError } from "@voya/client/toast-store";
import { i18next } from "@voya/i18n/core";

/**
 * The single app-wide TanStack Query client.
 *
 * Same reasoning as the desktop client (`apps/desktop/src/components/app-shell/
 * query-client.ts`): command failures are deterministic typed `AppError`
 * values, so retrying only delays the error UI, and the backend pushes
 * invalidation events, which makes refetch-on-focus redundant chatter.
 *
 * `refetchOnWindowFocus` is a DOM concept with no React Native equivalent, so
 * it is simply absent rather than disabled. Coming back from the background is
 * handled by `AppState` where a screen needs it, not globally.
 */
export function createMobileQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        staleTime: 30_000,
      },
    },
    mutationCache: new MutationCache({
      onError: (error, _variables, _onMutateResult, mutation) => {
        toastError(mutationErrorTitle(mutation), error);
      },
    }),
  });
}

function mutationErrorTitle(mutation: Mutation<unknown, unknown, unknown>) {
  const errorTitle = mutation.meta?.errorTitle;

  return typeof errorTitle === "string" && errorTitle.length > 0
    ? errorTitle
    : String(i18next.t("status.operationFailed"));
}
