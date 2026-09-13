import { useEffect } from "react";
import { FileWarning, LoaderCircle, X } from "lucide-react";
import { Button } from "@voya/ui/components/button";
import { InlinePageError } from "@/components/app-shell/inline-page-error";
import { PageSurface } from "@/components/app-shell/page-section";
import { getErrorMessage } from "@voya/utils/error";
import type { NodeNoticesController } from "./node-controller-types";

/** Long enough to read an import summary, short enough not to linger. */
const SUCCESS_NOTICE_MS = 8000;

export function ServerTableNotices({ controller }: { controller: NodeNoticesController }) {
  const {
    directImportPending,
    operationError,
    operationMessage,
    profilesQuery,
    setOperationError,
    setOperationMessage,
    t,
    undecodableProfiles,
  } = controller;
  // A success line clears itself. An error stays until it is dismissed or
  // replaced, because it usually names something the user still has to fix.
  useEffect(() => {
    if (!operationMessage) return undefined;
    const timer = setTimeout(() => setOperationMessage(null), SUCCESS_NOTICE_MS);
    return () => clearTimeout(timer);
  }, [operationMessage, setOperationMessage]);

  return (
    <>
      {directImportPending ? (
        <PageSurface className="flex shrink-0 items-center gap-2 px-4 py-2 text-sm text-muted-foreground" role="status" aria-busy="true">
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
          {directImportPending === "qrScreen" ? t("qr.scanningScreen") : directImportPending === "import" ? t("panes.profiles.import.importing") : t("panes.profiles.importDialog.reading")}
        </PageSurface>
      ) : null}
      {operationError ? (
        <InlinePageError>
          <span className="flex items-start gap-2">
            <span className="min-w-0 flex-1 whitespace-pre-line">{operationError}</span>
            <DismissButton label={t("common.dismiss")} onClick={() => setOperationError(null)} />
          </span>
        </InlinePageError>
      ) : null}
      {profilesQuery.isError ? (
        <InlinePageError>
          {getErrorMessage(profilesQuery.error)}
        </InlinePageError>
      ) : null}
      {operationMessage ? (
        <PageSurface role="status" className="flex shrink-0 items-start gap-2 bg-connected/10 px-4 py-2 text-sm text-success">
          <span className="min-w-0 flex-1">{operationMessage}</span>
          <DismissButton label={t("common.dismiss")} onClick={() => setOperationMessage(null)} />
        </PageSurface>
      ) : null}
      {/*
        Stored profiles this build could not decode are skipped by persistence so
        that one of them cannot hide every other server. This band is the only
        place the user hears about it: a toast would fire on every refetch of the
        list, so the shortfall is stated calmly next to the rows instead, and it
        stays until the profiles become readable again.
      */}
      {undecodableProfiles > 0 ? (
        <PageSurface
          className="flex shrink-0 items-start gap-2 bg-muted/40 px-4 py-2 text-sm text-muted-foreground"
          data-slot="profiles-undecodable-notice"
          role="status"
        >
          <FileWarning aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
          <span>
            {t("panes.profiles.undecodable", {
              count: undecodableProfiles.toLocaleString(),
            })}
          </span>
        </PageSurface>
      ) : null}
    </>
  );
}

function DismissButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <Button
      aria-label={label}
      className="-my-1 shrink-0 text-current"
      onClick={onClick}
      size="icon-xs"
      title={label}
      type="button"
      variant="ghost"
    >
      <X aria-hidden="true" className="size-3.5" />
    </Button>
  );
}
