import { useCallback, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { loadAppSettings, saveAppSettings } from "@/ipc";
import type {
  AppSettingsV1,
  AppearanceSettings,
} from "@/ipc/bindings";
import { getErrorMessage } from "@voya/utils/error";

import { applyUiPreferences, UI_PREFERENCES_QUERY_KEY } from "./ui-preferences";

const PREFERENCES_STORAGE_KEY = "voyavpn.preferences";
const LOCALE_STORAGE_KEY = "voyavpn.locale";
const APP_SETTINGS_QUERY_KEY = ["app-settings"] as const;

export type AppSettingsController = {
  settings: AppSettingsV1 | null;
  dirty: boolean;
  discard: () => Promise<void>;
  error: string | null;
  reload: () => Promise<void>;
  save: () => Promise<boolean>;
  saved: boolean;
  setAppearance: (preferences: AppearanceSettings) => void;
  update: (
    updater: (current: AppSettingsV1) => AppSettingsV1,
  ) => void;
  working: boolean;
};

export function useAppSettings(): AppSettingsController {
  const queryClient = useQueryClient();
  const settingsQuery = useQuery({
    queryFn: loadAppSettings,
    queryKey: APP_SETTINGS_QUERY_KEY,
  });
  const [draft, setDraft] = useState<AppSettingsV1 | null>(null);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [saving, setSaving] = useState(false);
  const original = settingsQuery.data ?? null;
  const settings = draft ?? original;

  const load = useCallback(async () => {
    setOperationError(null);
    setDraft(null);
    setSaved(false);
    const result = await settingsQuery.refetch();
    if (result.error) {
      setOperationError(getErrorMessage(result.error));
    }
  }, [settingsQuery]);

  const dirty = Boolean(draft && original && !settingsEqual(draft, original));

  const update = useCallback(
    (updater: (current: AppSettingsV1) => AppSettingsV1) => {
      setSaved(false);
      setDraft((current) => {
        const next = current ?? settingsQuery.data;
        return next ? updater(next) : current;
      });
    },
    [settingsQuery.data],
  );

  const setAppearance = useCallback(
    (preferences: AppearanceSettings) => {
      update((current) => ({ ...current, appearance: preferences }));
      void applyUiPreferencesPreview(preferences).catch((previewError: unknown) => {
        setOperationError(getErrorMessage(previewError));
      });
    },
    [update],
  );

  const discard = useCallback(async () => {
    if (!original) {
      return;
    }
    setDraft(null);
    setSaved(false);
    setOperationError(null);
    await applyUiPreferences(original.appearance).catch((rollbackError: unknown) => {
      setOperationError(getErrorMessage(rollbackError));
    });
  }, [original]);

  const save = useCallback(async () => {
    if (!settings) {
      return false;
    }
    setSaving(true);
    setOperationError(null);
    setSaved(false);
    try {
      const authoritative = await saveAppSettings(settings);
      queryClient.setQueryData(APP_SETTINGS_QUERY_KEY, authoritative);
      setDraft(null);
      setSaved(true);
      await applyUiPreferences(authoritative.appearance);
      queryClient.setQueryData(UI_PREFERENCES_QUERY_KEY, authoritative.appearance);
      return true;
    } catch (saveError) {
      setOperationError(getErrorMessage(saveError));
      try {
        const authoritative = await loadAppSettings();
        queryClient.setQueryData(APP_SETTINGS_QUERY_KEY, authoritative);
        setDraft(null);
        await applyUiPreferences(authoritative.appearance);
      } catch {
        // Keep the original save error; a later Reload can retry the snapshot.
      }
      return false;
    } finally {
      setSaving(false);
    }
  }, [settings, queryClient]);

  return {
    settings,
    dirty,
    discard,
    error: operationError ?? (settingsQuery.error ? getErrorMessage(settingsQuery.error) : null),
    reload: load,
    save,
    saved,
    setAppearance,
    update,
    working: saving || settingsQuery.isPending || settingsQuery.isFetching,
  };
}

async function applyUiPreferencesPreview(preferences: AppearanceSettings) {
  const stored = new Map(
    [PREFERENCES_STORAGE_KEY, LOCALE_STORAGE_KEY].map((key) => [
      key,
      window.localStorage.getItem(key),
    ]),
  );
  try {
    await applyUiPreferences(preferences);
  } finally {
    for (const [key, value] of stored) {
      if (value === null) {
        window.localStorage.removeItem(key);
      } else {
        window.localStorage.setItem(key, value);
      }
    }
  }
}

function settingsEqual(left: AppSettingsV1, right: AppSettingsV1) {
  return JSON.stringify(left) === JSON.stringify(right);
}
