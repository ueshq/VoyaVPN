import { type NavigationAction, useNavigation, usePreventRemove } from "@react-navigation/native";
import { useI18n } from "@voya/i18n/use-i18n";
import { useEffect, useState } from "react";
import { Alert } from "react-native";

/**
 * Asks before a page with unsaved edits is left: keep editing, discard, or
 * save and go.
 *
 * "Save" cannot leave from the save's own continuation. That runs before the
 * page has rendered what the save changed, so the navigator still holds the
 * callback of a page that is saving — and that callback, rightly, lets nothing
 * leave. The leave is remembered instead and carried out by the render that
 * shows the page clean.
 */
export function useUnsavedChanges(dirty: boolean, saving: boolean, save: () => Promise<boolean>) {
  const navigation = useNavigation();
  const { t } = useI18n();
  const [leaving, setLeaving] = useState<NavigationAction | null>(null);
  // Edited again while the save ran: the page is not clean after all, and the
  // user is back to editing it — a later save from its own button must not
  // spring this leave on them.
  if (leaving !== null && !saving && dirty) setLeaving(null);

  usePreventRemove(dirty || saving, ({ data }) => {
    if (saving) return;
    Alert.alert(t("mobile.unsaved"), t("mobile.leave"), [
      { text: t("mobile.keepEditing"), style: "cancel" },
      { text: t("mobile.discard"), style: "destructive", onPress: () => navigation.dispatch(data.action) },
      // A save that rejects is one that did not save: the page stays, and its
      // own error line says why. Left unhandled it would be a red screen.
      {
        text: t("actions.save"),
        onPress: () => {
          void save().then(
            (saved) => {
              if (saved) setLeaving(data.action);
            },
            () => undefined,
          );
        },
      },
    ]);
  });

  useEffect(() => {
    if (leaving !== null && !dirty && !saving) navigation.dispatch(leaving);
  }, [dirty, leaving, navigation, saving]);
}
