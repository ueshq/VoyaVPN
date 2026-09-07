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
import { useI18n } from "@voya/i18n/use-i18n";

import { RoutingProfileDialog } from "./routing-profile-dialog";
import { RoutingRuleDialog } from "./routing-rule-dialog";
import type { RoutingScreenController } from "./use-routing-screen";

export function RoutingDialogs({ controller }: { controller: RoutingScreenController }) {
  const { t } = useI18n();
  const {
    confirmDelete,
    handleSaveRouting,
    handleSaveRule,
    pendingDelete,
    routingDialog,
    ruleDialog,
    selectedRouting,
    setPendingDelete,
    setRoutingDialog,
    setRuleDialog,
  } = controller;
  const deletingRouting = pendingDelete === "routing";

  return (
    <>
      <RoutingProfileDialog
        key={routingDialog?.mode === "edit" ? `routing-${routingDialog.routing.id}` : `routing-${routingDialog?.mode ?? "closed"}`}
        mode={routingDialog?.mode ?? "create"}
        onOpenChange={(open) => !open && setRoutingDialog(null)}
        onSubmit={handleSaveRouting}
        open={Boolean(routingDialog)}
        routing={routingDialog?.mode === "edit" ? routingDialog.routing : null}
      />
      <RoutingRuleDialog
        key={ruleDialog?.mode === "edit" ? `rule-${ruleDialog.rule.id}` : `rule-${ruleDialog?.mode ?? "closed"}`}
        mode={ruleDialog?.mode ?? "create"}
        onOpenChange={(open) => !open && setRuleDialog(null)}
        onSubmit={handleSaveRule}
        open={Boolean(ruleDialog)}
        rule={ruleDialog?.mode === "edit" ? ruleDialog.rule : null}
      />
      {/* Deleting a routing profile also deletes every rule it owns, and both
          Delete buttons are enabled as soon as anything exists, so a misclick
          would be unrecoverable. Same AlertDialog gate the profiles table uses. */}
      <AlertDialog open={pendingDelete !== null} onOpenChange={(open) => !open && setPendingDelete(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {deletingRouting ? t("confirm.deleteRoutingTitle") : t("confirm.deleteRoutingRuleTitle")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {deletingRouting
                ? t("confirm.deleteRoutingDescription", {
                    count: selectedRouting?.rules.length ?? 0,
                    name: selectedRouting?.remarks || t("panes.routing.untitled"),
                  })
                : t("confirm.deleteRoutingRuleDescription")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction className={buttonVariants({ variant: "destructive" })} onClick={confirmDelete}>
              {deletingRouting ? t("confirm.deleteRoutingConfirm") : t("confirm.deleteRoutingRuleConfirm")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
