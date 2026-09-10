import { useEffect, useState, useSyncExternalStore } from "react";
import { useQueryClient, type QueryKey } from "@tanstack/react-query";
import { z } from "zod";

import { IpcCommandError } from "@/ipc";
import { validationFieldErrors } from "@/ipc/messages";
import { translateFieldErrors, zodIssuesToErrorMap } from "@/lib/zod-errors";
import { i18next, type TranslationFunction } from "@voya/i18n";
import { getErrorMessage } from "@voya/utils/error";
import { useToastStore } from "@/stores/toast-store";

import { applyChanges, SettingsDraft, type SettingsChange } from "./settings-draft";
import { settingsSaveQueue } from "./settings-save-queue";

// Async failures use the current locale even after the originating pane unmounts.
const t: TranslationFunction = i18next.t.bind(i18next);

export function useSettingsDraft<T>({ data, queryKey, write }: {
  data: T | undefined;
  queryKey: QueryKey;
  write: (change: SettingsChange) => Promise<T>;
}) {
  const client = useQueryClient();
  const queue = settingsSaveQueue(client);
  const [draft] = useState(() => new SettingsDraft<T>({
    read: () => client.getQueryData<T>(queryKey),
    write,
    enqueue: (key, job) => queue.enqueue(key, job),
    failure: (error) => {
      if (error instanceof z.ZodError) {
        return { message: t("validation.dnsSettings"), fields: translateFieldErrors(t, zodIssuesToErrorMap(error)) };
      }
      return {
        message: getErrorMessage(error),
        fields: error instanceof IpcCommandError && error.appError.kind.type === "validation"
          ? validationFieldErrors(t, error.appError.kind.issues) : {},
      };
    },
    report: ({ message }) => {
      useToastStore.getState().pushToast({ title: t("settings.autosave.failed"), description: message, severity: "error" });
    },
  }));
  const snapshot = useSyncExternalStore(draft.subscribe, draft.getSnapshot, draft.getSnapshot);
  const saving = useSyncExternalStore(queue.subscribe, queue.isSaving, queue.isSaving);

  useEffect(() => { draft.attach(); return draft.detach; }, [draft]);

  const failures = Object.values(snapshot.failures);
  return {
    value: data ? applyChanges(data, snapshot.changes) : null,
    error: failures.length ? failures[0].message : null,
    fieldErrors: Object.assign({}, ...failures.map((failure) => failure.fields)) as Record<string, string>,
    retry: draft.retry,
    saved: snapshot.saved && snapshot.changes.length === 0,
    saving,
    update: draft.update,
  };
}
