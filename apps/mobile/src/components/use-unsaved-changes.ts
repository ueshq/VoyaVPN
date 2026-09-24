import { useNavigation, usePreventRemove } from "@react-navigation/native";
import { useI18n } from "@voya/i18n/use-i18n";
import { Alert } from "react-native";

export function useUnsavedChanges(dirty: boolean, saving: boolean, save: () => Promise<boolean>) {
  const navigation = useNavigation();
  const { t } = useI18n();
  usePreventRemove(dirty || saving, ({ data }) => {
    if (saving) return;
    Alert.alert(t("mobile.unsaved"), t("mobile.leave"), [
      { text: t("mobile.keepEditing"), style: "cancel" },
      { text: t("mobile.discard"), style: "destructive", onPress: () => navigation.dispatch(data.action) },
      { text: t("actions.save"), onPress: () => { void save().then((saved) => { if (saved) navigation.dispatch(data.action); }); } },
    ]);
  });
}
