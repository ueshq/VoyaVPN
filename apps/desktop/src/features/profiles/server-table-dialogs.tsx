import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@voya/ui/components/alert-dialog";
import { buttonVariants } from "@voya/ui/components/button-variants";
import { SubscriptionsDialog } from "@/features/subscriptions";

import { ImportProfilesDialog } from "./import-profiles-dialog";
import { ProfileDialog } from "./profile-dialog";
import { ShareQrDialog } from "./share-qr-dialog";
import type { ServerTableController } from "./use-server-table";

export function ServerTableDialogs({ controller }: { controller: ServerTableController }) {
  const {
    confirmDedupe,
    confirmDelete,
    dialogState,
    handleDialogImport,
    handleSave,
    importOpen,
    pendingDedupe,
    pendingDelete,
    queryClient,
    saveError,
    setDialogState,
    setImportOpen,
    setPendingDedupe,
    setPendingDelete,
    setShareQrContent,
    setSubscriptionsOpen,
    shareQrContent,
    subscriptionsOpen,
    t,
  } = controller;

  return (
    <>
      <ProfileDialog
        mode={dialogState?.mode ?? "create"}
        onOpenChange={(open) => !open && setDialogState(null)}
        onSubmit={handleSave}
        open={Boolean(dialogState)}
        profile={dialogState?.mode === "edit" ? dialogState.profile : null}
        saveError={saveError}
      />
      <ImportProfilesDialog
        onImported={handleDialogImport}
        onOpenChange={setImportOpen}
        open={importOpen}
      />
      <SubscriptionsDialog
        onChanged={() => void queryClient.invalidateQueries({ queryKey: ["profiles"] })}
        onOpenChange={setSubscriptionsOpen}
        open={subscriptionsOpen}
      />
      <ShareQrDialog
        content={shareQrContent ?? ""}
        onOpenChange={(open) => !open && setShareQrContent(null)}
        open={shareQrContent !== null}
      />
      <AlertDialog open={pendingDedupe} onOpenChange={(open) => !open && setPendingDedupe(false)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("confirm.dedupeProfilesTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("confirm.dedupeProfilesDescription")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={buttonVariants({ variant: "destructive" })}
              onClick={() => void confirmDedupe()}
            >
              {t("confirm.dedupeProfilesConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <AlertDialog open={pendingDelete !== null} onOpenChange={(open) => !open && setPendingDelete(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("confirm.deleteProfilesTitle")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t("confirm.deleteProfilesDescription", { count: pendingDelete?.length ?? 0 })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction className={buttonVariants({ variant: "destructive" })} onClick={confirmDelete}>
              {t("confirm.deleteProfilesConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
