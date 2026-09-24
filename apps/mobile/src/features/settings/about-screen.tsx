import { useQuery } from "@tanstack/react-query";
import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "heroui-native/button";
import { Typography } from "heroui-native/text";
import { Platform } from "react-native";
import { useState } from "react";
import { DetailScreen } from "~/components/detail-screen";
import { appVersion } from "~/native/device-actions";
import notices from "~/generated/legal-notices.json";
import { openPage } from "~/app/navigation";

export function AboutScreen() {
  const { t } = useI18n();
  const version = useQuery({ queryKey: ["mobile", "version"], queryFn: appVersion });
  const [licenses, setLicenses] = useState(false);
  return <DetailScreen>
    <Typography accessibilityRole="header" className="text-2xl font-semibold text-foreground">VoyaVPN {version.data ?? ""}</Typography>
    <Typography accessibilityRole="header" className="text-lg font-semibold text-foreground">{t("mobile.help")}</Typography>
    <Typography className="text-base text-subtle">{t("mobile.helpText")}</Typography>
    <Button variant="secondary" className="min-h-12 h-auto" onPress={() => openPage("logs")}><Button.Label>{t("mobile.diagnostics")}</Button.Label></Button>
    <Typography accessibilityRole="header" className="text-lg font-semibold text-foreground">{t("mobile.privacy")}</Typography>
    <Typography className="text-base text-subtle">{t("mobile.privacyText")}</Typography>
    {Platform.OS === "android" ? <Typography className="text-base text-subtle">{t("mobile.androidScannerPrivacy")}</Typography> : null}
    <Button variant="secondary" className="min-h-12 h-auto" onPress={() => setLicenses(!licenses)} accessibilityState={{ expanded: licenses }}><Button.Label>{t("mobile.licenses")}</Button.Label></Button>
    {licenses ? <Typography selectable className="text-sm text-foreground">{notices.license}{"\n\n"}{notices.thirdParty}</Typography> : null}
  </DetailScreen>;
}
