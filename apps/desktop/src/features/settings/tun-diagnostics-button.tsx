import { useState } from "react";
import { ClipboardCopy, LoaderCircle } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { useI18n } from "@voya/i18n/use-i18n";
import { tunProviderDiagnostics } from "@/ipc";
import type { TunProviderDiagnostics } from "@/ipc/bindings";
import { getErrorMessage } from "@voya/utils/error";
import { useMountedRef } from "@voya/utils/use-mounted-ref";
import { useToastStore } from "@/stores/toast-store";

/**
 * Copies a structured TUN provider diagnostics report to the clipboard. Moved
 * from the removed bottom status bar into the Settings → Network TUN group,
 * next to the options it helps debug.
 */
export function TunDiagnosticsButton() {
  const { t } = useI18n();
  const pushToast = useToastStore((state) => state.pushToast);
  const mountedRef = useMountedRef();
  const [copying, setCopying] = useState(false);
  const label = t("status.copyTunDiagnostics");

  async function copyTunDiagnostics() {
    if (copying) {
      return;
    }

    setCopying(true);
    try {
      if (typeof navigator === "undefined" || !navigator.clipboard?.writeText) {
        throw new Error(t("status.copyTunDiagnosticsClipboardUnavailable"));
      }

      const diagnostics = await tunProviderDiagnostics();
      await navigator.clipboard.writeText(formatTunDiagnosticsForClipboard(diagnostics));
      pushToast({
        description: t("status.copyTunDiagnosticsCopied"),
        severity: "info",
        title: label,
      });
    } catch (error) {
      pushToast({
        description: getErrorMessage(error),
        severity: "error",
        title: t("status.copyTunDiagnosticsFailed"),
      });
    } finally {
      if (mountedRef.current) {
        setCopying(false);
      }
    }
  }

  return (
    <Button
      className="w-fit gap-2"
      disabled={copying}
      onClick={() => void copyTunDiagnostics()}
      size="sm"
      type="button"
      variant="outline"
    >
      {copying ? (
        <LoaderCircle className="size-4 animate-spin" aria-hidden="true" />
      ) : (
        <ClipboardCopy className="size-4" aria-hidden="true" />
      )}
      {label}
    </Button>
  );
}

function formatTunDiagnosticsForClipboard(diagnostics: TunProviderDiagnostics) {
  return JSON.stringify(
    {
      type: "voya.tunProviderDiagnostics",
      backend: diagnostics.backend,
      packagingMode: diagnostics.packagingMode,
      systemExtensionState: diagnostics.systemExtensionState,
      status: {
        state: diagnostics.statusState,
        lastError: diagnostics.lastError,
        message: diagnostics.message,
      },
      paths: {
        container: diagnostics.containerPath,
        status: diagnostics.statusPath,
        log: diagnostics.logPath,
        providerBundle: diagnostics.providerBundlePath,
        expectedProvider: diagnostics.expectedProviderPath,
      },
      registrationPaths: diagnostics.registrationPaths,
      breadcrumbs: diagnostics.breadcrumbs,
      providerLogTail: diagnostics.providerLogTail,
      hostLogTail: diagnostics.hostLogTail,
    },
    null,
    2,
  );
}
