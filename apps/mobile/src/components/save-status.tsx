import { useI18n } from "@voya/i18n/use-i18n";
import { Typography } from "heroui-native/text";

/**
 * Where an editor's edits stand: being saved, not yet saved, or saved.
 *
 * Announced as it changes, because it is the only word a screen reader gets
 * that a save went through. `saved` is for a page that knows a save happened
 * in this visit — one that opened already saying "Saved" would teach the user
 * to ignore the line.
 */
export function SaveStatus({ dirty, saved = false, saving }: { dirty: boolean; saved?: boolean; saving: boolean }) {
  const { t } = useI18n();

  return (
    <Typography accessibilityLiveRegion="polite" className="text-sm text-subtle">
      {saving ? t("settings.saveStatus.saving") : dirty ? t("mobile.unsaved") : saved ? t("mobile.saved") : null}
    </Typography>
  );
}
