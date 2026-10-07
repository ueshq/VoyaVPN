import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "heroui-native/button";
import { ListGroup } from "heroui-native/list-group";
import { Typography } from "heroui-native/text";
import { useState } from "react";
import { Platform } from "react-native";
import { SwitchRow } from "~/components/switch-row";
import { Disclosure } from "~/components/disclosure";
import { ErrorNotice } from "~/components/error-notice";
import { deviceActions } from "~/native/device-actions";
import { useConnectionPreferences } from "@voya/client/connection-preferences";

export function ConnectionOptions() {
  const { t } = useI18n();
  const autoConnect = useConnectionPreferences((s) => s.autoConnect);
  const [error, setError] = useState<unknown>(null);
  const [installed, setInstalled] = useState(false);
  const [busy, setBusy] = useState(false);
  async function install() {
    setBusy(true); setError(null); setInstalled(false);
    try {
      await deviceActions().setConnectionShortcuts(t("actions.connect"), t("actions.disconnect"));
      setInstalled(true);
    } catch (failure) { setError(failure); }
    finally { setBusy(false); }
  }
  return <>
    <ListGroup><SwitchRow last label={t("daily.autoConnect")} description={t("daily.autoConnectHint")} value={autoConnect} onChange={(enabled) => useConnectionPreferences.getState().setAutoConnect(enabled)} /></ListGroup>
    {Platform.OS !== "android" || Number(Platform.Version) >= 25 ? <Disclosure title={t("daily.shortcuts")}>
      <Typography className="text-base text-subtle">{t("daily.shortcutsHint")}</Typography>
      <ErrorNotice error={error} />
      <Button variant="secondary" isDisabled={busy} onPress={() => void install()}><Button.Label>{t("daily.shortcutsInstall")}</Button.Label></Button>
      {installed ? <Typography accessibilityLiveRegion="polite" className="text-sm text-connected">{t("daily.shortcutsReady")}</Typography> : null}
    </Disclosure> : null}
  </>;
}
