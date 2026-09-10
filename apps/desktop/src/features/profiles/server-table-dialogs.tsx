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
import { SubscriptionsDialog } from "@/features/subscriptions/subscriptions-dialog";

import { ImportProfilesDialog } from "./import-profiles-dialog";
import { ProfileDetailsDialog } from "./profile-details-dialog";
import { ProfileDialog } from "./profile-dialog";
import { ShareQrDialog } from "./share-qr-dialog";
import type { ServerTableController } from "./use-server-table";

export function ServerTableDialogs({ controller }: { controller: ServerTableController }) {
  const {
    confirmDelete,
    dialogState,
    handleDialogImport,
    handleSave,
    importMethod,
    pendingDelete,
    saveError,
    setDialogState,
    setImportMethod,
    setPendingDelete,
    setShareQrContent,
    setSubscriptionsOpen,
    shareQrContent,
    subscriptionsOpen,
    t,
  } = controller;

  const detailsItem = controller.profiles.find((item) => item.profile.id === controller.detailsId);

  return (
    <>
      {detailsItem ? <ProfileDetailsDialog controller={controller} item={detailsItem} /> : null}
      <ProfileDialog
        onCloseFocus={controller.restoreProfileDialogFocus}
        mode={dialogState?.mode ?? "create"}
        onOpenChange={(open) => !open && setDialogState(null)}
        onSubmit={handleSave}
        open={Boolean(dialogState)}
        profile={dialogState?.mode === "edit" ? dialogState.profile : null}
        saveError={saveError}
      />
      <ImportProfilesDialog
        onImported={handleDialogImport}
        method={importMethod ?? "text"}
        onCloseFocus={() => controller.importTriggerRef.current?.focus()}
        onOpenChange={(open) => !open && setImportMethod(null)}
        open={importMethod !== null}
      />
      <SubscriptionsDialog
        initialMode="create"
        onCloseFocus={() => controller.addTriggerRef.current?.focus()}
        onOpenChange={setSubscriptionsOpen}
        open={subscriptionsOpen}
      />
      <ShareQrDialog
        content={shareQrContent ?? ""}
        onOpenChange={(open) => !open && setShareQrContent(null)}
        open={shareQrContent !== null}
      />
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
