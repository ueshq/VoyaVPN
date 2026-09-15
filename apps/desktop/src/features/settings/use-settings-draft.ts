import { useEffect, useState, useSyncExternalStore } from "react";
import {
  useQueryClient,
  type QueryClient,
  type QueryKey,
} from "@tanstack/react-query";
import { z } from "zod";

import { appErrorOfKind } from "@/ipc/commands";
import { validationFieldErrors } from "@/ipc/messages";
import { translateFieldErrors, zodIssuesToErrorMap } from "@/lib/zod-errors";
import { i18next, type TranslationFunction } from "@voya/i18n";
import { getErrorMessage } from "@voya/utils/error";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { toastError } from "@/stores/toast-store";

import {
  applyChanges,
  SettingsDraft,
  type SettingsChange,
} from "./settings-draft";
import { settingsSaveQueue } from "./settings-save-queue";

// Async failures use the current locale even after the originating pane unmounts.
const draftsByClient = new WeakMap<QueryClient, Map<string, unknown>>();

const t: TranslationFunction = i18next.t.bind(i18next);

export function useSettingsDraft<T>({
  data,
  queryKey,
  write,
}: {
  data: T | undefined;
  queryKey: QueryKey;
  write: (change: SettingsChange) => Promise<T>;
}) {
  const client = useQueryClient();
  const queue = settingsSaveQueue(client);
  const [draft] = useState(() => {
    const key = JSON.stringify(queryKey);
    const drafts = draftsByClient.get(client) ?? new Map<string, unknown>();
    draftsByClient.set(client, drafts);
    const existing = drafts.get(key) as SettingsDraft<T> | undefined;
    if (existing) return existing;
    const created = new SettingsDraft<T>({
      read: () => client.getQueryData<T>(queryKey),
      write,
      enqueue: (key, job) => queue.enqueue(key, job),
      failure: (error) => {
        if (error instanceof z.ZodError) {
          return {
            message: t("validation.dnsSettings"),
            fields: translateFieldErrors(t, zodIssuesToErrorMap(error)),
          };
        }
        const validation = appErrorOfKind(error, "validation");
        return {
          message: redactOperationalError(error),
          fields: validation ? validationFieldErrors(t, validation.kind.issues) : {},
        };
      },
      report: ({ message }) => {
        toastError(t("settings.autosave.failed"), message);
      },
    });
    drafts.set(key, created);
    return created;
  });
  const snapshot = useSyncExternalStore(
    draft.subscribe,
    draft.getSnapshot,
    draft.getSnapshot,
  );
  const saving = useSyncExternalStore(
    queue.subscribe,
    queue.isSaving,
    queue.isSaving,
  );

  useEffect(() => {
    draft.attach();
    return draft.detach;
  }, [draft]);

  const failures = Object.values(snapshot.failures);
  return {
    value: data ? applyChanges(data, snapshot.changes) : null,
    error: failures.length ? failures[0].message : null,
    fieldErrors: Object.assign(
      {},
      ...failures.map((failure) => failure.fields),
    ) as Record<string, string>,
    retry: draft.retry,
    saved: snapshot.saved && snapshot.changes.length === 0,
    saving,
    update: draft.update,
  };
}

/**
 * A settings pane's failure: the draft's save error, else the failed read.
 * Retrying resends the failed saves and reads again after a failed load.
 */
export function settingsFailure(
  draft: { error: string | null; retry: () => void },
  query: { error: unknown; isError: boolean; refetch: () => Promise<unknown> },
) {
  return {
    error: draft.error ?? (query.error ? getErrorMessage(query.error) : null),
    retry: () => {
      draft.retry();
      if (query.isError) void query.refetch();
    },
  };
}
