import { queries } from "@voya/client/queries";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { voyaCommands } from "@voya/client/transport";
import type { AppSettings, DnsSettings } from "@voya/contracts";
import { queryKeys } from "@voya/client/query-keys";
import { applyChanges, changedFields } from "@voya/features/settings/settings-draft";
import { settingsFailure, useSettingsDraft } from "@voya/features/settings/use-settings-draft";

import { dnsSettingsSchema } from "@voya/features/dns/dns-form-schema";

export function useDnsSettings(enabled = true) {
  const client = useQueryClient();
  const dnsQuery = useQuery({ ...queries.dns, enabled, refetchOnMount: "always" });
  const draft = useSettingsDraft<DnsSettings>({
    data: dnsQuery.data,
    queryKey: queryKeys.dns,
    write: async (change) => {
      const baseline = await voyaCommands().loadDnsSettings();
      const next = dnsSettingsSchema.parse(applyChanges(baseline, [change]));
      const saved = changedFields(baseline, next).length ? await voyaCommands().saveDnsSettings(next) : baseline;
      await Promise.all([
        client.cancelQueries({ queryKey: queryKeys.dns }),
        client.cancelQueries({ queryKey: queryKeys.appSettings }),
      ]);
      client.setQueryData(queryKeys.dns, saved);
      client.setQueryData<AppSettings>(queryKeys.appSettings, (current) => current ? { ...current, dns: saved } : current);
      return saved;
    },
  });

  const failure = settingsFailure(draft, dnsQuery);
  return {
    dnsQuery,
    fieldErrors: draft.fieldErrors,
    form: draft.value,
    issueCount: Object.keys(draft.fieldErrors).length,
    operationError: failure.error,
    retry: failure.retry,
    saved: draft.saved,
    saving: draft.saving,
    updateSimple: (patch: Partial<DnsSettings>) => draft.update((current) => ({ ...current, ...patch })),
  };
}
