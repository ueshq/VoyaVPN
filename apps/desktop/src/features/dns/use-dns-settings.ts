import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { z } from "zod";

import { IpcCommandError, loadDnsSettings, saveDnsSettings } from "@/ipc";
import type { DnsSettings } from "@/ipc/bindings";
import { getErrorMessage } from "@voya/utils/error";

import { dnsSettingsSchema, zodIssuesToErrorMap } from "./dns-form-schema";

export function useDnsSettings() {
  const queryClient = useQueryClient();
  const dnsQuery = useQuery({
    queryFn: loadDnsSettings,
    queryKey: ["dns"],
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
    await queryClient.invalidateQueries({ queryKey: ["dns"] });
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
      queryClient.setQueryData(["dns"], saved);
      setDraft(null);
      await queryClient.invalidateQueries({ queryKey: ["dns"] });
      await queryClient.invalidateQueries({ queryKey: ["app-config"] });
      return null;
    } catch (error) {
      if (error instanceof z.ZodError) {
        const message = "DNS settings validation failed";
        setOperationError(message);
        setFieldErrors(zodIssuesToErrorMap(error));
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
