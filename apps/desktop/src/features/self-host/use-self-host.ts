import { useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { useI18n } from "@voya/i18n/use-i18n";
import { redactOperationalError } from "@voya/utils/operational-redaction";

import type { SelfHostConfig, SelfHostState } from "@voya/contracts";
import { appErrorOfKind } from "@voya/client/errors";
import { voyaCommands } from "@voya/client/transport";
import { validationFieldErrors } from "@voya/client/messages";
import { queryKeys } from "@voya/client/query-keys";
import { toastError } from "@voya/client/toast-store";

/** Live counters refresh this often while the node runs and the page is open. */
const STATS_INTERVAL_MS = 3000;

export type SelfHostAction = "enable" | "save" | "rotate" | "check" | "firewall";

/**
 * The Self-hosted node page's data and actions.
 *
 * Every command answers with the whole page state, which replaces the cache at
 * once; the `selfHost` invalidation the backend also emits keeps other windows
 * and the background watch loop in step. Settings saves are applied to the
 * cache first and sent one at a time, so two quick toggles compose instead of
 * the second overwriting the first.
 */
export function useSelfHost() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const stateQuery = useQuery({
    queryFn: () => voyaCommands().getSelfHostState(),
    queryKey: queryKeys.selfHost,
  });
  const state = stateQuery.data ?? null;
  const running = state?.runtime.status === "running";
  const statsQuery = useQuery({
    enabled: running,
    queryFn: () => voyaCommands().getSelfHostStats(),
    queryKey: queryKeys.selfHostStats,
    refetchInterval: running ? STATS_INTERVAL_MS : false,
  });
  const [pending, setPending] = useState<SelfHostAction | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const chain = useRef<Promise<unknown>>(Promise.resolve());

  function publish(next: SelfHostState) {
    queryClient.setQueryData(queryKeys.selfHost, next);
  }

  async function run(action: SelfHostAction, operation: () => Promise<SelfHostState>) {
    setPending(action);
    try {
      publish(await operation());
      setFieldErrors({});
      return true;
    } catch (error) {
      const validation = appErrorOfKind(error, "validation");
      if (validation) {
        setFieldErrors(validationFieldErrors(t, validation.kind.issues));
      } else {
        toastError(t("panes.selfHost.actionFailed"), error);
      }
      // The optimistic value may be wrong now; the backend has the truth.
      await queryClient.invalidateQueries({ queryKey: queryKeys.selfHost });
      return false;
    } finally {
      setPending(null);
    }
  }

  /** Queues `operation` behind any change still in flight. */
  function enqueue(action: SelfHostAction, operation: () => Promise<SelfHostState>) {
    const next = chain.current.then(() => run(action, operation));
    chain.current = next;
    return next;
  }

  function saveConfig(patch: Partial<SelfHostConfig>) {
    const current = queryClient.getQueryData<SelfHostState>(queryKeys.selfHost);
    if (!current) return Promise.resolve(false);
    publish({ ...current, config: { ...current.config, ...patch } });
    // Composed when it is sent, not now: a change queued ahead of it may fail,
    // and then the cache holds the backend's config instead of the rejected one.
    return enqueue("save", () => {
      const latest = queryClient.getQueryData<SelfHostState>(queryKeys.selfHost) ?? current;
      return voyaCommands().saveSelfHostConfig({ ...latest.config, ...patch });
    });
  }

  function setEnabled(enabled: boolean) {
    const current = queryClient.getQueryData<SelfHostState>(queryKeys.selfHost);
    if (current) publish({ ...current, config: { ...current.config, enabled } });
    return enqueue("enable", () => voyaCommands().setSelfHostEnabled(enabled));
  }

  return {
    fieldErrors,
    loadError: stateQuery.error ? redactOperationalError(stateQuery.error) : null,
    pending,
    state,
    stats: running ? (statsQuery.data ?? null) : null,
    applyFirewallRule: () => enqueue("firewall", () => voyaCommands().applySelfHostFirewallRule()),
    rotateCredentials: () => enqueue("rotate", () => voyaCommands().rotateSelfHostCredentials()),
    runCheck: () => enqueue("check", () => voyaCommands().runSelfHostEnvironmentCheck()),
    saveConfig,
    setEnabled,
  };
}

export type SelfHostController = ReturnType<typeof useSelfHost>;
