import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";

import { IpcCommandError, loadDnsSettings, saveDnsSettings } from "@/ipc";
import type { DnsSettings } from "@/ipc/bindings";
import { validationText } from "@/ipc/messages";
import { queryKeys } from "@/ipc/query-keys";
import { useI18n } from "@voya/i18n/use-i18n";
import { getErrorMessage } from "@voya/utils/error";
import { translateFieldErrors, zodIssuesToErrorMap } from "@/lib/zod-errors";

import { dnsSettingsSchema } from "./dns-form-schema";

export function useDnsSettings() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const dnsQuery = useQuery({
    queryFn: loadDnsSettings,
    queryKey: queryKeys.dns,
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
    // User-initiated Reload with no backend mutation behind it, so nothing
    // emits an invalidation for it.
    await queryClient.invalidateQueries({ queryKey: queryKeys.dns });
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
      // Optimistic: the pane keeps showing what it just saved while
      // `save_dns_settings`'s `dns` + `app-settings` invalidation lands. That
      // bundle invalidation is what stops a later Save-all from reposting the
      // pre-save DNS block.
      queryClient.setQueryData(queryKeys.dns, saved);
      setDraft(null);
      return null;
    } catch (error) {
      if (error instanceof z.ZodError) {
        const message = t("validation.dnsSettings");
        setOperationError(message);
        setFieldErrors(translateFieldErrors(t, zodIssuesToErrorMap(error)));
        return message;
      }
      if (error instanceof IpcCommandError && error.appError.kind.type === "validation") {
        // `appError.message` is the backend's English diagnostic; the banner
        // and every field message come from the locale files instead.
        const message = t("validation.dnsSettings");
        setOperationError(message);
        setFieldErrors(
          Object.fromEntries(
            error.appError.kind.issues.map((issue) => [issue.field, validationText(t, issue)]),
          ),
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
