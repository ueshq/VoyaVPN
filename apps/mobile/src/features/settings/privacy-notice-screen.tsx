import { usePreferencesStore } from "@voya/client/preferences-store";
import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "heroui-native/button";
import { LinkButton } from "heroui-native/link-button";
import { Typography } from "heroui-native/text";
import { useRef } from "react";
import { AccessibilityInfo, ScrollView, View, findNodeHandle, type Text } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useContentColumn } from "~/components/content-column";
import { openExternalLink } from "~/native/external-link";

import { PRIVACY_NOTICE_VERSION, PRIVACY_POLICY_URL, SUPPORT_URL } from "./privacy-notice";

const STATEMENTS = [
  "mobile.privacyNoticeDevice",
  "mobile.privacyNoticeTraffic",
  "mobile.privacyNoticeRequests",
  "mobile.privacyNoticeDiagnostics",
] as const;

function NoticeLink({ label, testID, url }: { label: string; testID: string; url: string }) {
  return (
    <LinkButton
      testID={testID}
      accessibilityRole="link"
      className="min-h-control"
      // The address rides along for VoiceOver instead of being printed again
      // under the link: one blue line, not a line and its grey echo.
      accessibilityLabel={`${label}: ${url}`}
      onPress={() => void openExternalLink(url)}
    >
      <LinkButton.Label className="text-accent">{label}</LinkButton.Label>
    </LinkButton>
  );
}

/**
 * The statements and links of the data notice (see `privacy-notice.ts`). The
 * first-run screen and About both show them.
 */
export function PrivacyNoticeContent() {
  const { t } = useI18n();
  return (
    <View className="gap-3">
      <Typography className="text-base text-foreground">{t("mobile.privacyNoticeIntro")}</Typography>
      {STATEMENTS.map((key) => (
        <View key={key} className="flex-row gap-2">
          <Typography accessible={false} className="text-base text-subtle">•</Typography>
          <Typography className="flex-1 text-base text-foreground">{t(key)}</Typography>
        </View>
      ))}
      <NoticeLink label={t("mobile.privacyPolicy")} testID="privacy-policy-link" url={PRIVACY_POLICY_URL} />
      <NoticeLink label={t("mobile.support")} testID="support-link" url={SUPPORT_URL} />
    </View>
  );
}

/**
 * The first-run screen. The shell renders it in place of navigation, so there
 * is nothing behind it to reach: no backdrop to tap and no back gesture.
 */
export function PrivacyNoticeScreen() {
  const { t } = useI18n();
  const insets = useSafeAreaInsets();
  const column = useContentColumn();
  const titleRef = useRef<Text>(null);

  function focusTitle() {
    const target = findNodeHandle(titleRef.current);
    if (target != null) AccessibilityInfo.setAccessibilityFocus(target);
  }

  return (
    <View accessibilityViewIsModal className="flex-1 bg-canvas">
      <ScrollView
        contentContainerClassName="gap-6 px-page"
        contentContainerStyle={[{ paddingBottom: insets.bottom + 24, paddingTop: insets.top + 24 }, column]}
      >
        <Typography ref={titleRef} onLayout={focusTitle} accessibilityRole="header" className="text-3xl font-bold text-foreground">
          {t("mobile.privacyNoticeTitle")}
        </Typography>
        <PrivacyNoticeContent />
        <Button
          testID="privacy-continue"
          size="lg"
          onPress={() => usePreferencesStore.getState().acceptPrivacyNotice(PRIVACY_NOTICE_VERSION)}
        >
          <Button.Label>{t("mobile.privacyContinue")}</Button.Label>
        </Button>
      </ScrollView>
    </View>
  );
}
