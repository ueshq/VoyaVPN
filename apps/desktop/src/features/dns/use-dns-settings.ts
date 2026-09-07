import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";

import { IpcCommandError, loadDnsSettings, saveDnsSettings } from "@/ipc";
import type { DnsSettings } from "@/ipc/bindings";
import { APP_SETTINGS_QUERY_KEY, DNS_QUERY_KEY } from "@/lib/query-keys";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { translateFieldErrors, zodIssuesToErrorMap } from "@/lib/zod-errors";

import { dnsSettingsSchema } from "./dns-form-schema";

export function useDnsSettings() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const dnsQuery = useQuery({
    queryFn: loadDnsSettings,
    queryKey: DNS_QUERY_KEY,
  });
  const [draft, setDraft] = useState<DnsSettings | null>(null);
  const [operationError, setOperationError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const form = draft ?? dnsQuery.data ?? null;

  const issueCount = Object.keys(fieldErrors).length;
  const isDirty = useMemo(() => {
    if (!form || !dnsQuery.data) {
      return false;
    }
    return draft !== null && JSON.stringify(draft) !== JSON.stringify(dnsQuery.data);
  }, [dnsQuery.data, draft, form]);

  async function handleReload() {
    setOperationError(null);
    setFieldErrors({});
    setDraft(null);
    await queryClient.invalidateQueries({ queryKey: DNS_QUERY_KEY });
  }

  /** Resolves to the failure message, or null once the draft is persisted. */
  async function handleSave(): Promise<string | null> {
    if (!form) {
      return null;
    }
    setOperationError(null);
    setFieldErrors({});
    try {
      const payload = dnsSettingsSchema.parse(form);
      const saved = await saveDnsSettings(payload);
      queryClient.setQueryData(DNS_QUERY_KEY, saved);
      setDraft(null);
      await queryClient.invalidateQueries({ queryKey: DNS_QUERY_KEY });
      await queryClient.invalidateQueries({ queryKey: ["app-config"] });
      // Save-all posts the whole settings bundle, DNS block included. Without
      // this the Settings controller keeps serving the pre-save snapshot and a
      // later Save-all silently reverts what was just written here.
      await queryClient.invalidateQueries({ queryKey: APP_SETTINGS_QUERY_KEY });
      return null;
    } catch (error) {
      if (error instanceof z.ZodError) {
        const message = t("validation.dnsSettings");
        setOperationError(message);
        setFieldErrors(translateFieldErrors(t, zodIssuesToErrorMap(error)));
        return message;
      }
      if (error instanceof IpcCommandError && error.appError.kind === "dns") {
        const message = error.appError.message.message;
        setOperationError(message);
        setFieldErrors(
          Object.fromEntries(error.appError.message.issues.map((issue) => [issue.field, issue.message])),
        );
        return message;
      }
      const message = getErrorMessage(error);
      setOperationError(message);
      return message;
    }
  }

  function updateSimple(patch: Partial<DnsSettings>) {
    setDraft((current) =>
      current
        ? {
            ...current,
            ...patch,
          }
        : dnsQuery.data
          ? {
              ...dnsQuery.data,
              ...patch,
            }
          : current,
    );
  }

  return {
    dnsQuery,
    fieldErrors,
    form,
    handleReload,
    handleSave,
    isDirty,
    issueCount,
    operationError,
    updateSimple,
  };
}
