import { useQuery, useQueryClient } from "@tanstack/react-query";

import { loadDnsSettings, saveDnsSettings } from "@/ipc/commands";
import type { AppSettingsV1, DnsSettings } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { getErrorMessage } from "@voya/utils/error";
import { applyChanges, changedFields } from "@/features/settings/settings-draft";
import { useSettingsDraft } from "@/features/settings/use-settings-draft";

import { dnsSettingsSchema } from "./dns-form-schema";

export function useDnsSettings(enabled = true) {
  const client = useQueryClient();
  const dnsQuery = useQuery({ enabled, queryFn: loadDnsSettings, queryKey: queryKeys.dns, refetchOnMount: "always" });
  const draft = useSettingsDraft<DnsSettings>({
    data: dnsQuery.data,
    queryKey: queryKeys.dns,
    write: async (change) => {
      const baseline = await loadDnsSettings();
      const next = dnsSettingsSchema.parse(applyChanges(baseline, [change]));
      const saved = changedFields(baseline, next).length ? await saveDnsSettings(next) : baseline;
      await Promise.all([
        client.cancelQueries({ queryKey: queryKeys.dns }),
        client.cancelQueries({ queryKey: queryKeys.appSettings }),
      ]);
      client.setQueryData(queryKeys.dns, saved);
      client.setQueryData<AppSettingsV1>(queryKeys.appSettings, (current) => current ? { ...current, dns: saved } : current);
      return saved;
    },
  });

  return {
    dnsQuery,
    fieldErrors: draft.fieldErrors,
    form: draft.value,
    issueCount: Object.keys(draft.fieldErrors).length,
    operationError: draft.error ?? (dnsQuery.error ? getErrorMessage(dnsQuery.error) : null),
    retry: () => { draft.retry(); if (dnsQuery.isError) void dnsQuery.refetch(); },
    saved: draft.saved,
    saving: draft.saving,
    updateSimple: (patch: Partial<DnsSettings>) => draft.update((current) => ({ ...current, ...patch })),
  };
}
