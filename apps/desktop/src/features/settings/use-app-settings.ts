import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { loadAppSettings, saveAppSettings } from "@/ipc";
import type { AppSettingsV1, AppearanceSettings } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { getErrorMessage } from "@voya/utils/error";

import { applyChanges, changedFields } from "./settings-draft";
import { useSettingsDraft } from "./use-settings-draft";
import { applyUiPreferences, endUiPreferencesPreview, previewUiPreferences, reportUiPreferencesError } from "./ui-preferences";

export function useAppSettings() {
  const client = useQueryClient();
  const [previewOwner] = useState(() => Symbol("settings appearance"));
  const query = useQuery({ queryFn: loadAppSettings, queryKey: queryKeys.appSettings, refetchOnMount: "always" });
  const draft = useSettingsDraft<AppSettingsV1>({
    data: query.data,
    queryKey: queryKeys.appSettings,
    write: async (change) => {
      // Construct the complete IPC DTO at dispatch, after earlier app/DNS writes.
      // Failed or still-being-edited fields are never included in this snapshot.
      const baseline = await loadAppSettings();
      const next = applyChanges(baseline, [change]);
      const saved = changedFields(baseline, next).length ? await saveAppSettings(next) : baseline;
      await Promise.all([
        client.cancelQueries({ queryKey: queryKeys.appSettings }),
        client.cancelQueries({ queryKey: queryKeys.dns }),
        client.cancelQueries({ queryKey: queryKeys.uiPreferences }),
      ]);
      client.setQueryData(queryKeys.appSettings, saved);
      client.setQueryData(queryKeys.dns, saved.dns);
      client.setQueryData(queryKeys.uiPreferences, saved.appearance);
      await applyUiPreferences(saved.appearance).catch(reportUiPreferencesError);
      return saved;
    },
  });

  const appearance = draft.value?.appearance;
  const authoritative = query.data?.appearance;
  useEffect(() => {
    if (appearance && authoritative && changedFields(authoritative, appearance).length === 0) {
      endUiPreferencesPreview(previewOwner);
    }
  }, [appearance, authoritative, previewOwner]);

  useEffect(() => () => {
    endUiPreferencesPreview(previewOwner);
    const saved = client.getQueryData<AppSettingsV1>(queryKeys.appSettings);
    if (saved) void applyUiPreferences(saved.appearance).catch(reportUiPreferencesError);
  }, [client, previewOwner]);

  function setAppearance(preferences: AppearanceSettings) {
    previewUiPreferences(previewOwner, preferences);
    draft.update((current) => ({ ...current, appearance: preferences }));
  }

  return {
    settings: draft.value,
    error: draft.error ?? (query.error ? getErrorMessage(query.error) : null),
    fieldErrors: draft.fieldErrors,
    retry: () => { draft.retry(); if (query.isError) void query.refetch(); },
    saved: draft.saved,
    saving: draft.saving,
    setAppearance,
    update: draft.update,
    working: query.isPending,
  };
}

export type AppSettingsController = ReturnType<typeof useAppSettings>;

export type AppSettingsFormController = AppSettingsController & { settings: AppSettingsV1 };
