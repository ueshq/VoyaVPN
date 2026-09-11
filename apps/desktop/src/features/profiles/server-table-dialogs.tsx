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

import { NodeGroupDialogs } from "./node-group-dialogs";
import { ImportProfilesDialog } from "./import-profiles-dialog";
import { ProfileDetailsDialog } from "./profile-details-dialog";
import { ProfileDialog } from "./profile-dialog";
import { ShareQrDialog } from "./share-qr-dialog";
import type { NodeDialogsController } from "./node-controller-types";

export function ServerTableDialogs({
  controller,
}: {
  controller: NodeDialogsController;
}) {
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

  const detailsItem = controller.profiles.find(
    (item) => item.profile.id === controller.detailsId,
  );

  return (
    <>
      <NodeGroupDialogs controller={controller} />
      {detailsItem ? (
        <ProfileDetailsDialog controller={controller} item={detailsItem} />
      ) : null}
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
        subscription={controller.editingSubscription}
        onCloseFocus={() =>
          (controller.subscriptionTriggerRef.current?.isConnected
            ? controller.subscriptionTriggerRef.current
            : controller.viewportRef.current
          )?.focus()
        }
        onOpenChange={setSubscriptionsOpen}
        open={subscriptionsOpen}
      />
      <ShareQrDialog
        content={shareQrContent ?? ""}
        onOpenChange={(open) => !open && setShareQrContent(null)}
        open={shareQrContent !== null}
      />
      <AlertDialog
        open={!!controller.deletingSubscription}
        onOpenChange={(open) =>
          !open &&
          !controller.deletingSubscriptionPending &&
          controller.setDeletingSubscription(null)
        }
      >
        <AlertDialogContent
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            (controller.subscriptionTriggerRef.current?.isConnected
              ? controller.subscriptionTriggerRef.current
              : controller.viewportRef.current
            )?.focus();
          }}
        >
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("subscriptions.deleteTitle", {
                name: controller.deletingSubscription?.remarks,
              })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("subscriptions.deleteHint", {
                count: controller.profiles.filter(
                  (entry) =>
                    entry.profile.subscriptionId ===
                    controller.deletingSubscription?.id,
                ).length,
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {controller.operationError ? (
            <p role="alert" className="text-sm text-danger">
              {controller.operationError}
            </p>
          ) : null}
          <AlertDialogFooter>
            <AlertDialogCancel
              disabled={controller.deletingSubscriptionPending}
            >
              {t("confirm.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              disabled={controller.deletingSubscriptionPending}
              className={buttonVariants({ variant: "destructive" })}
              onClick={(event) => {
                event.preventDefault();
                void controller.removeSubscription();
              }}
            >
              {t("confirm.deleteProfilesConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
      <AlertDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => !open && setPendingDelete(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("confirm.deleteProfilesTitle")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("confirm.deleteProfilesDescription", {
                count: pendingDelete?.length ?? 0,
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={buttonVariants({ variant: "destructive" })}
              onClick={confirmDelete}
            >
              {t("confirm.deleteProfilesConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
