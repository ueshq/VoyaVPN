import { useRef, useState } from "react";
import { clipboard } from "@voya/client/platform";
import { voyaCommands } from "@voya/client/transport";
import { importLineText } from "@voya/client/messages";
import type { ImportProfilesResult, QrScanFailureReason } from "@voya/contracts";
import type { TranslationFunction, TranslationKey } from "@voya/i18n/core";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { useMountedRef } from "@voya/utils/use-mounted-ref";
import type { DirectImportMethod } from "./import-methods";
import type { NodeOperation } from "./use-node-operation";

const SCREEN_FAILURE_KEYS = {
  permissionDenied: "qr.screenPermissionDenied",
  unsupported: "qr.screenUnavailable",
  captureFailed: "qr.screenCaptureFailed",
  timeout: "qr.screenTimeout",
  busy: "qr.screenBusy",
} satisfies Record<QrScanFailureReason, TranslationKey>;

function emptyResult(): ImportProfilesResult {
  return {
    imported: 0, updated: 0, skipped: 0, parsed: 0, filtered: 0, deduped: 0,
    failed: 0, removedExisting: 0, removedDuplicates: 0, discardedNodeOverrides: 0,
    subscriptionId: null, importedProfileIds: [], updatedProfileIds: [], lineIssues: [],
    addedSubscriptionIds: [],
  };
}

function mergeResult(total: ImportProfilesResult, next: ImportProfilesResult) {
  const previousIds = new Set(total.importedProfileIds);
  const repeated = next.importedProfileIds.filter((id) => previousIds.has(id)).length;
  total.importedProfileIds = [...new Set([...total.importedProfileIds, ...next.importedProfileIds])];
  total.updatedProfileIds = [...new Set([
    ...total.updatedProfileIds,
    ...next.updatedProfileIds.filter((id) => !previousIds.has(id)),
  ])];
  total.imported = total.importedProfileIds.length;
  total.updated = total.updatedProfileIds.length;
  total.skipped += next.skipped + repeated;
  total.deduped += next.deduped + repeated;
  total.parsed += next.parsed;
  total.filtered += next.filtered;
  total.failed += next.failed;
  total.removedExisting += next.removedExisting;
  total.removedDuplicates += next.removedDuplicates;
  total.discardedNodeOverrides += next.discardedNodeOverrides;
  total.lineIssues.push(...next.lineIssues);
  total.addedSubscriptionIds = [
    ...new Set([...total.addedSubscriptionIds, ...next.addedSubscriptionIds]),
  ];
}

export function useNodeImport(
  { setOperationError, setOperationMessage }: NodeOperation,
  onImported: (result: ImportProfilesResult, isActive: () => boolean) => Promise<void>,
  t: TranslationFunction,
) {
  const [directImportPending, setDirectImportPending] = useState<DirectImportMethod | "import" | null>(null);
  const pendingRef = useRef(false);
  const activeRef = useMountedRef();

  async function handleDirectImport(method: DirectImportMethod) {
    if (pendingRef.current || !activeRef.current) return;
    pendingRef.current = true;
    setDirectImportPending(method);
    setOperationError(null);
    setOperationMessage(null);
    const isActive = () => activeRef.current;
    const issues: string[] = [];
    try {
      let payloads: string[];
      if (method === "clipboard") {
        // Read through the platform clipboard: a WebView read asks the user to
        // confirm "Paste" every time, so the desktop reads it natively.
        const text = (await clipboard().readText().catch(() => {
          throw new Error(t("panes.profiles.import.clipboardUnavailable"));
        })).trim();
        if (!text) throw new Error(t("panes.profiles.import.clipboardEmpty"));
        payloads = [text];
      } else {
        const scanned = await voyaCommands().scanScreenQr();
        if (!isActive()) return;
        if (scanned.failureReason) issues.push(t(SCREEN_FAILURE_KEYS[scanned.failureReason]));
        payloads = scanned.status === "found" ? scanned.texts : [];
        if (payloads.length === 0) {
          throw new Error(issues.join(" ") || t(scanned.status === "unavailable" ? "qr.screenUnavailable" : "qr.noQrFound"));
        }
      }
      if (!isActive()) return;
      setDirectImportPending("import");
      const total = emptyResult();
      const messages: string[] = [];
      // Preserve individual JSON/Base64 payload boundaries; persistence handles node identity.
      const uniquePayloads = [...new Set(payloads.map((payload) => payload.trim()).filter(Boolean))];
      if (uniquePayloads.length === 0) throw new Error(t("qr.noQrFound"));
      for (const payload of uniquePayloads) {
        if (!isActive()) return;
        try {
          const result = await voyaCommands().importProfilesFromText(payload, null);
          mergeResult(total, result);
          messages.push(
            ...result.lineIssues.map((issue) => redactOperationalError(importLineText(t, issue))),
          );
        } catch (error) {
          total.failed += 1;
          messages.push(redactOperationalError(error));
        }
      }
      if (!isActive()) return;
      issues.push(...messages);
      try {
        await onImported(total, isActive);
      } catch (error) {
        issues.push(redactOperationalError(error));
      }
      if (isActive() && issues.length > 0) setOperationError([...new Set(issues)].join("\n"));
    } catch (error) {
      if (isActive()) setOperationError(redactOperationalError(error));
    } finally {
      pendingRef.current = false;
      if (isActive()) setDirectImportPending(null);
    }
  }

  return { directImportPending, handleDirectImport };
}
