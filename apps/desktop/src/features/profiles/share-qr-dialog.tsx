import { useMemo } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { AlertTriangle, QrCode } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogDescription,
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { Label } from "@voya/ui/components/label";
import { Textarea } from "@voya/ui/components/textarea";
import { cn } from "@voya/ui/lib/utils";
import { getErrorMessage } from "@voya/utils/error";
import { generateQrCode } from "@/ipc/commands";
import { profileShareQrQueryKey } from "@voya/client/query-keys";

type ShareQrDialogProps = {
  content: string;
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

/**
 * The QR code of `content`, drawn by the backend. Nothing is requested while
 * `enabled` is false or the content is blank.
 *
 * The frame keeps its size from the first render, and a new `content` keeps
 * the previous code up (dimmed) until its own arrives, so switching between
 * links never empties the frame and the dialog around it never jumps.
 */
export function ShareQrImage({
  className,
  content,
  enabled = true,
}: {
  className?: string;
  content: string;
  enabled?: boolean;
}) {
  const { t } = useI18n();
  const qrCodeQuery = useQuery({
    enabled: enabled && content.trim().length > 0,
    gcTime: 0,
    placeholderData: keepPreviousData,
    queryFn: () => generateQrCode(content),
    queryKey: profileShareQrQueryKey(content),
    refetchOnWindowFocus: false,
    retry: false,
    staleTime: Infinity,
  });
  const imageSource = useMemo(() => {
    if (!qrCodeQuery.data) {
      return null;
    }

    return `data:${qrCodeQuery.data.mimeType};utf8,${encodeURIComponent(qrCodeQuery.data.svg)}`;
  }, [qrCodeQuery.data]);

  if (qrCodeQuery.isError) {
    return (
      <Alert variant="destructive">
        <AlertTriangle aria-hidden="true" />
        <AlertDescription>
          {getErrorMessage(qrCodeQuery.error)}
        </AlertDescription>
      </Alert>
    );
  }

  const sizeClassName = cn("size-64 max-w-full", className);
  return (
    <div className="grid justify-items-center rounded-md border bg-background p-4">
      {imageSource ? (
        <img
          alt={t("qr.generatedAlt")}
          className={cn(
            sizeClassName,
            "transition-opacity duration-short",
            qrCodeQuery.isPlaceholderData && "opacity-50",
          )}
          src={imageSource}
        />
      ) : (
        <div aria-hidden="true" className={sizeClassName} />
      )}
    </div>
  );
}

export function ShareQrDialog({
  content,
  onOpenChange,
  open,
}: ShareQrDialogProps) {
  const { t } = useI18n();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <ScrollableDialogContent
        height="viewport"
        width="lg"
        closeLabel={t("actions.close")}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <QrCode className="size-4" aria-hidden="true" />
            {t("panes.profiles.export.showQr")}
          </DialogTitle>
          <DialogDescription className="sr-only">
            {t("panes.profiles.export.shareLinks")}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <div className="grid gap-4">
            <div className="grid gap-1">
              <Label
                className="text-xs text-muted-foreground"
                htmlFor="profile-share-qr-content"
              >
                {t("qr.content")}
              </Label>
              <Textarea
                className="min-h-24 resize-y bg-card font-mono text-xs"
                id="profile-share-qr-content"
                readOnly
                value={content}
              />
            </div>

            <ShareQrImage content={content} enabled={open} />
          </div>
        </DialogBody>
        <DialogFooter>
          <Button
            onClick={() => onOpenChange(false)}
            type="button"
            variant="outline"
          >
            {t("actions.close")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}
