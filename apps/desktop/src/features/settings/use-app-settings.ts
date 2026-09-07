import { useCallback, useState } from "react";
import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";

import { loadAppSettings, saveAppSettings } from "@/ipc";
import type {
  AppDnsSettings,
  AppSettingsV1,
  AppearanceSettings,
} from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { getErrorMessage } from "@voya/utils/error";

import { applyUiPreferences } from "./ui-preferences";

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
    queryKey: queryKeys.appSettings,
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
      // Preview only: the appearance is not persisted until Save-all succeeds.
      void applyUiPreferences(preferences, { persist: false }).catch((previewError: unknown) => {
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
      const authoritative = await saveAppSettings(withFreshestDns(settings, queryClient));
      queryClient.setQueryData(queryKeys.appSettings, authoritative);
      setDraft(null);
      setSaved(true);
      await applyUiPreferences(authoritative.appearance);
      queryClient.setQueryData(queryKeys.uiPreferences, authoritative.appearance);
      return true;
    } catch (saveError) {
      setOperationError(getErrorMessage(saveError));
      try {
        const authoritative = await loadAppSettings();
        queryClient.setQueryData(queryKeys.appSettings, authoritative);
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

/**
 * No Settings tab edits `settings.dns`: the DNS pane writes the same backend
 * field through its own command pair. This controller seeds its draft once from
 * the cached bundle (`update()` uses `current ?? settingsQuery.data`), so a DNS
 * save made after that seeding would be silently reverted by Save-all — and,
 * because the backend compares `simple_dns_item` to decide on a restart, the
 * core would be restarted with the reverted resolvers. Always post the freshest
 * DNS block instead of the draft's snapshot of it.
 */
function withFreshestDns(settings: AppSettingsV1, queryClient: QueryClient): AppSettingsV1 {
  // The DNS pane writes this cache synchronously when its own save succeeds, so
  // it is the authoritative DNS block whenever the pane has been used at all.
  // `DnsSettings` (its DTO) and `AppDnsSettings` (the bundle's block) are the
  // same shape; the backend maps one onto the other.
  const dns = queryClient.getQueryData<AppDnsSettings>(queryKeys.dns);

  return dns ? { ...settings, dns } : settings;
}

function settingsEqual(left: AppSettingsV1, right: AppSettingsV1) {
  return JSON.stringify(left) === JSON.stringify(right);
}
