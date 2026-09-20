import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { useI18n } from "@voya/i18n/use-i18n";
import { SettingsFields } from "@/features/settings/settings-form";
import { TestsTab } from "@/features/settings/tests-tab";
import { useAppSettings } from "@voya/features/settings/use-app-settings";

/**
 * The latency test's own settings, next to the tests they change. They save
 * automatically, like every other setting.
 */
export function SpeedtestSettingsDialog({
  onOpenChange,
}: {
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useI18n();
  const controller = useAppSettings();
  const { settings } = controller;

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <ScrollableDialogContent closeLabel={t("actions.close")} width="40rem">
        <DialogHeader>
          <DialogTitle>{t("panes.profiles.speedtest.settings")}</DialogTitle>
          <DialogDescription>{t("panes.profiles.speedtest.settingsDescription")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="grid gap-4">
          {controller.error ? (
            <p className="text-sm text-danger" role="alert">
              {controller.error}
            </p>
          ) : null}
          <SettingsFields errors={controller.fieldErrors}>
            {settings ? (
              <TestsTab controller={{ ...controller, settings }} />
            ) : (
              <p className="text-sm text-muted-foreground" role="status">
                {t("options.loading")}
              </p>
            )}
          </SettingsFields>
        </DialogBody>
        {/* The fields save themselves; the footer says so and gives a way out. */}
        <DialogFooter className="items-center sm:justify-between">
          <p className="text-xs text-muted-foreground" role="status">
            {controller.saving
              ? t("panes.profiles.speedtest.saving")
              : controller.saved
                ? t("panes.profiles.speedtest.saved")
                : null}
          </p>
          <Button onClick={() => onOpenChange(false)} type="button">
            {t("actions.done")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}
