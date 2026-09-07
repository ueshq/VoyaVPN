import { useEffect, useLayoutEffect, useRef, useState } from "react";

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
import {
  discardSettingsDirtySources,
  saveSettingsDirtySources,
  useSettingsDirtySources,
} from "./settings-dirty-sources";
import { useAppSettings } from "./use-app-settings";

/**
 * The in-shell Settings destination. Owns the app-settings controller and a
 * navigation leave guard: switching to another shell tab while any settings
 * draft is dirty parks the target in `pendingTab` and asks to save or discard
 * first, mirroring the close guard of the former dedicated settings window.
 * Panes that keep their own draft (the DNS tab) are covered through the dirty
 * source registry — leaving the tab unmounts them, so an unguarded switch
 * would discard their edits silently.
 */
export function SettingsScreen() {
  const { t } = useI18n();
  const controller = useAppSettings();
  const paneDirty = useSettingsDirtySources();
  const pendingTab = useShellStore((state) => state.pendingTab);
  const [paneError, setPaneError] = useState<string | null>(null);
  const dirty = controller.dirty || paneDirty;
  const dialogError = controller.error ?? paneError;
  const dirtyRef = useRef(dirty);

  useLayoutEffect(() => {
    dirtyRef.current = dirty;
  }, [dirty]);

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

  async function saveAll(): Promise<boolean> {
    setPaneError(null);
    if (controller.dirty && !(await controller.save())) {
      return false;
    }
    const error = await saveSettingsDirtySources();
    setPaneError(error);
    return error === null;
  }

  async function discardAll() {
    setPaneError(null);
    if (controller.dirty) {
      await controller.discard();
    }
    await discardSettingsDirtySources();
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
              error line and the DNS pane's own alert sit behind the modal
              overlay where they cannot be seen. */}
          {dialogError ? (
            <p className="text-xs text-destructive" role="alert">
              {dialogError}
            </p>
          ) : null}
          <AlertDialogFooter>
            <AlertDialogCancel>{t("confirm.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              onClick={(event) => {
                event.preventDefault();
                void discardAll().then(resolvePendingNavigation);
              }}
            >
              {t("settings.discardChanges")}
            </AlertDialogAction>
            <AlertDialogAction
              onClick={(event) => {
                event.preventDefault();
                void saveAll().then((saved) => {
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
