import { useEffect, useLayoutEffect, useRef } from "react";

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
import { PageSection, PageTitle } from "@/components/app-shell/page-section";
import { useI18n } from "@voya/i18n/use-i18n";
import { useShellStore } from "@/stores/shell-store";

import { SettingsSurface } from "./settings-dialog";
import { useAppSettings } from "./use-app-settings";

/**
 * The in-shell Settings destination. Owns the app-settings controller and a
 * navigation leave guard: switching to another shell tab while the draft is
 * dirty parks the target in `pendingTab` and asks to save or discard first,
 * mirroring the close guard of the former dedicated settings window.
 */
export function SettingsScreen() {
  const { t } = useI18n();
  const controller = useAppSettings();
  const pendingTab = useShellStore((state) => state.pendingTab);
  const dirtyRef = useRef(controller.dirty);

  useLayoutEffect(() => {
    dirtyRef.current = controller.dirty;
  }, [controller.dirty]);

  useEffect(() => {
    useShellStore.getState().setNavigationGuard(() => !dirtyRef.current);
    return () => {
      useShellStore.getState().setNavigationGuard(null);
      useShellStore.getState().clearPendingTab();
    };
  }, []);

  function resolvePendingNavigation() {
    const target = useShellStore.getState().pendingTab;
    if (target) {
      useShellStore.getState().setActiveTab(target);
    }
  }

  return (
    <PageSection aria-label={t("modal.settings")}>
      <PageTitle title={t("modal.settings")} />
      <SettingsSurface controller={controller} />

      <AlertDialog
        onOpenChange={(open) => {
          if (!open) useShellStore.getState().clearPendingTab();
        }}
        open={pendingTab !== null}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("settings.closeUnsavedTitle")}</AlertDialogTitle>
            <AlertDialogDescription>{t("settings.closeUnsavedDescription")}</AlertDialogDescription>
          </AlertDialogHeader>
          {/* Save failures must surface inside the dialog — the surface footer's
              error line sits behind the modal overlay where it cannot be seen. */}
          {controller.error ? (
            <p className="text-xs text-destructive" role="alert">
              {controller.error}
            </p>
          ) : null}
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              onClick={(event) => {
                event.preventDefault();
                void controller.discard().then(resolvePendingNavigation);
              }}
            >
              {t("settings.discardChanges")}
            </AlertDialogAction>
            <AlertDialogAction
              onClick={(event) => {
                event.preventDefault();
                void controller.save().then((saved) => {
                  if (saved) resolvePendingNavigation();
                });
              }}
            >
              {t("settings.saveAll")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </PageSection>
  );
}
