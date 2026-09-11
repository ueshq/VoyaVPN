import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, QrCode } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@voya/ui/components/dialog";
import { Label } from "@voya/ui/components/label";
import { Textarea } from "@voya/ui/components/textarea";
import { getErrorMessage } from "@voya/utils/error";
import { generateQrCode } from "@/ipc/commands";
import { profileShareQrQueryKey } from "@/ipc/query-keys";

type ShareQrDialogProps = {
  content: string;
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

export function ShareQrDialog({
  content,
  onOpenChange,
  open,
}: ShareQrDialogProps) {
  const { t } = useI18n();
  const qrCodeQuery = useQuery({
    enabled: open && content.trim().length > 0,
    gcTime: 0,
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

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-h-[calc(100dvh-2rem)] grid-rows-[auto_minmax(0,1fr)_auto]"
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

            {imageSource ? (
              <div className="grid justify-items-center rounded-md border bg-background p-4">
                <img
                  alt={t("qr.generatedAlt")}
                  className="size-64 max-w-full"
                  src={imageSource}
                />
              </div>
            ) : null}

            {qrCodeQuery.isError ? (
              <Alert variant="destructive">
                <AlertTriangle aria-hidden="true" />
                <AlertDescription>
                  {getErrorMessage(qrCodeQuery.error)}
                </AlertDescription>
              </Alert>
            ) : null}
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
      </DialogContent>
    </Dialog>
  );
}
