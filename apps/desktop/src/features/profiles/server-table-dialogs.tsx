import { ConfirmDialog } from "@voya/ui/components/confirm-dialog";
import { restoreFocus } from "@voya/ui/lib/focus";
import { SubscriptionsDialog } from "@/features/subscriptions/subscriptions-dialog";
import { runningProfileId, useRuntimeEventStore } from "@voya/client/runtime-event-store";

import { ImportProfilesDialog } from "./import-profiles-dialog";
import { ProfileDetailsDialog } from "./profile-details-dialog";
import { ProfileDialog } from "./profile-dialog";
import { ShareQrDialog } from "./share-qr-dialog";
import type { ServerTableController } from "./use-server-table";

export function ServerTableDialogs({
  controller,
}: {
  controller: ServerTableController;
}) {
  const {
    confirmDelete,
    dialogState,
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

  // Deleting the node the core is running stops the connection; say so first.
  const runningNodeId = useRuntimeEventStore((state) => runningProfileId(state.coreState));
  const deletingRunningNode =
    runningNodeId !== null && (pendingDelete ?? []).includes(runningNodeId);

  const detailsItem = controller.profiles.find(
    (item) => item.profile.id === controller.detailsId,
  );

  return (
    <>
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
      {importMethod !== null ? <ImportProfilesDialog
        onImported={controller.handleImported}
        onCloseFocus={() => controller.addTriggerRef.current?.focus()}
        onOpenChange={(open) => !open && setImportMethod(null)}
        open
      /> : null}
      <SubscriptionsDialog
        subscription={controller.editingSubscription}
        onCloseFocus={() =>
          restoreFocus(
            controller.subscriptionTriggerRef.current,
            controller.viewportRef.current,
          )
        }
        onOpenChange={setSubscriptionsOpen}
        open={subscriptionsOpen}
      />
      <ShareQrDialog
        content={shareQrContent ?? ""}
        onOpenChange={(open) => !open && setShareQrContent(null)}
        open={shareQrContent !== null}
      />
      <ConfirmDialog
        cancelLabel={t("confirm.cancel")}
        confirmLabel={t("confirm.deleteProfilesConfirm")}
        description={t("subscriptions.deleteHint", {
          count: controller.profiles.filter(
            (entry) =>
              entry.profile.subscriptionId ===
              controller.deletingSubscription?.id,
          ).length,
        })}
        destructive
        error={controller.operationError}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          restoreFocus(
            controller.subscriptionTriggerRef.current,
            controller.viewportRef.current,
          );
        }}
        onConfirm={(event) => {
          event.preventDefault();
          void controller.removeSubscription();
        }}
        onOpenChange={(open) =>
          !open &&
          !controller.deletingSubscriptionPending &&
          controller.setDeletingSubscription(null)
        }
        open={!!controller.deletingSubscription}
        pending={controller.deletingSubscriptionPending}
        title={t("subscriptions.deleteTitle", {
          name: controller.deletingSubscription?.remarks,
        })}
      />
      <ConfirmDialog
        cancelLabel={t("confirm.cancel")}
        confirmLabel={t("confirm.deleteProfilesConfirm")}
        description={
          <>
            {t("confirm.deleteProfilesDescription", {
              count: pendingDelete?.length ?? 0,
            })}
            {deletingRunningNode ? (
              <span className="mt-2 block font-medium text-warning">
                {t("confirm.deleteActiveProfileHint")}
              </span>
            ) : null}
          </>
        }
        destructive
        onConfirm={confirmDelete}
        onOpenChange={(open) => !open && setPendingDelete(null)}
        open={pendingDelete !== null}
        title={t("confirm.deleteProfilesTitle")}
      />
    </>
  );
}
