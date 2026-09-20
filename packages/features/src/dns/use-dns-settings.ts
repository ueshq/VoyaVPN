import { useQuery, useQueryClient } from "@tanstack/react-query";

import { voyaCommands } from "@voya/client/transport";
import type { AppSettingsV1, DnsSettings } from "@voya/contracts";
import { queryKeys } from "@voya/client/query-keys";
import { applyChanges, changedFields } from "../settings/settings-draft";
import { settingsFailure, useSettingsDraft } from "../settings/use-settings-draft";

import { dnsSettingsSchema } from "./dns-form-schema";

export function useDnsSettings(enabled = true) {
  const client = useQueryClient();
  const dnsQuery = useQuery({ enabled, queryFn: () => voyaCommands().loadDnsSettings(), queryKey: queryKeys.dns, refetchOnMount: "always" });
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
      client.setQueryData<AppSettingsV1>(queryKeys.appSettings, (current) => current ? { ...current, dns: saved } : current);
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
