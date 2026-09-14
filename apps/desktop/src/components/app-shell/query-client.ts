import { MutationCache, QueryClient, type Mutation } from "@tanstack/react-query";

import { i18next } from "@voya/i18n";
import { toastError } from "@/stores/toast-store";

/**
 * The single app-wide TanStack Query client.
 *
 * Defaults are tuned for a local IPC backend rather than a flaky network:
 * command failures are deterministic typed `AppError` values, so retrying only
 * delays the error UI, and the backend already pushes invalidation events
 * (ADR 0002 channel 1), which makes focus refetching redundant chatter.
 *
 * The mutation cache is the safety net that keeps a failed mutation from
 * disappearing silently: every rejection surfaces as a toast. A feature can
 * name the failure by passing `meta: { errorTitle: t("…") }` to `useMutation`;
 * without it the generic operation title is used.
 */
export function createAppQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        refetchOnWindowFocus: false,
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
