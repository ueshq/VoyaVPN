import { queries } from "@voya/client/queries";
import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { voyaCommands } from "@voya/client/transport";
import type { AppSettings, AppearanceSettings } from "@voya/contracts";
import { queryKeys } from "@voya/client/query-keys";
import { useLatestRef } from "@voya/utils/use-latest-ref";

import { applyChanges, changedFields } from "./settings-draft";
import { settingsFailure, useSettingsDraft } from "./use-settings-draft";
import { applyUiPreferences, endUiPreferencesPreview, previewUiPreferences, reportUiPreferencesError, reportUiPreferencesReverted } from "./ui-preferences";

export function useAppSettings() {
  const client = useQueryClient();
  const [previewOwner] = useState(() => Symbol("settings appearance"));
  const query = useQuery({ ...queries.appSettings, refetchOnMount: "always" });
  const draft = useSettingsDraft<AppSettings>({
    data: query.data,
    queryKey: queryKeys.appSettings,
    write: async (change) => {
      // Construct the complete IPC DTO at dispatch, after earlier app/DNS writes.
      // Failed or still-being-edited fields are never included in this snapshot.
      const baseline = await voyaCommands().loadAppSettings();
      const next = applyChanges(baseline, [change]);
      const saved = changedFields(baseline, next).length ? await voyaCommands().saveAppSettings(next) : baseline;
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

  const failed = useLatestRef(draft.error !== null);

  useEffect(() => () => {
    const previewed = endUiPreferencesPreview(previewOwner);
    const saved = client.getQueryData<AppSettings>(queryKeys.appSettings);
    if (!saved) return;
    // Leaving drops a preview whose save failed; say so instead of silently
    // switching the theme or language back.
    if (failed.current && previewed && changedFields(saved.appearance, previewed).length) {
      reportUiPreferencesReverted();
    }
    void applyUiPreferences(saved.appearance).catch(reportUiPreferencesError);
  }, [client, failed, previewOwner]);

  function setAppearance(preferences: AppearanceSettings) {
    previewUiPreferences(previewOwner, preferences);
    draft.update((current) => ({ ...current, appearance: preferences }));
  }

  const failure = settingsFailure(draft, query);
  return {
    settings: draft.value,
    error: failure.error,
    fieldErrors: draft.fieldErrors,
    retry: failure.retry,
    saved: draft.saved,
    saving: draft.saving,
    setAppearance,
    update: draft.update,
    working: query.isPending,
  };
}

export type AppSettingsController = ReturnType<typeof useAppSettings>;

export type AppSettingsFormController = AppSettingsController & { settings: AppSettings };
