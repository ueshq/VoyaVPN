import { ErrorNotice } from "~/components/error-notice";
import { openPage } from "~/app/navigation";
import { useAppSettings } from "@voya/features/settings/use-app-settings";
import { useRuleLibraryUpdate } from "@voya/features/updates/use-rule-library-update";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { localeOptions } from "@voya/i18n/core";
import { useI18n } from "@voya/i18n/use-i18n";
import type { ThemeMode } from "@voya/contracts";
import type { TranslationKey } from "@voya/i18n/core";
import { Button } from "heroui-native/button";
import { Card } from "heroui-native/card";
import { ListGroup } from "heroui-native/list-group";
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { Check, Database, ScrollText } from "lucide-react-native";
import { View } from "react-native";

import { DetailScreen } from "~/components/detail-screen";
import { IconBadge } from "~/components/icon-badge";
import { ListRow } from "~/components/list-row";
import { SectionHeader } from "~/components/section-header";
import { SegmentedControl } from "~/components/segmented-control";
import { SwitchRow } from "~/components/switch-row";
import { useToneColor } from "~/components/tone";

/** The three theme choices, in the order the desktop offers them. */
const THEME_MODES = [
  { labelKey: "menu.themeSystem", value: "system" },
  { labelKey: "menu.themeLight", value: "light" },
  { labelKey: "menu.themeDark", value: "dark" },
] as const satisfies readonly { labelKey: TranslationKey; value: ThemeMode }[];

/**
 * General: appearance, language, and the two behaviour switches that mean
 * something on a phone. Everything the desktop keeps under Advanced — the
 * capture mode, TUN diagnostics, autostart, the close action — is either a
 * system setting here or has no meaning at all (see `UNSUPPORTED_ON_MOBILE` in
 * the mobile host).
 */
export function GeneralScreen() {
  const { t } = useI18n();
  const checkColor = useToneColor("brand");
  const app = useAppSettings();
  const settings = app.settings;

  return (
    <DetailScreen gap="gap-6">
      {app.working ? <Spinner size="sm" /> : null}
      <ErrorNotice error={app.error} retry={app.retry} />

      {settings ? <>
        <View>
          <SectionHeader title={t("settings.sections.appearance")} />
          <Card className="gap-3 p-4">
            <Typography className="text-sm font-medium text-subtle">{t("modal.theme")}</Typography>
            <SegmentedControl
              options={THEME_MODES.map(({ labelKey, value }) => ({ label: t(labelKey), value }))}
              value={settings.appearance.theme}
              onChange={(theme) => app.setAppearance({ ...settings.appearance, theme })}
            />
          </Card>
        </View>

        <View>
          <SectionHeader title={t("modal.language")} />
          {/* Theme and language are the same kind of choice, so they go through
              the same write: `setAppearance` previews at once and persists on
              the backend's acknowledgement. */}
          <ListGroup>
            {localeOptions.map((locale, index) => {
              const selected = settings.appearance.language === locale.code;
              return (
                <ListRow
                  key={locale.code}
                  last={index === localeOptions.length - 1}
                  title={locale.nativeName}
                  trailing={selected ? <Check size={20} color={checkColor} accessible={false} /> : null}
                  onPress={() => app.setAppearance({ ...settings.appearance, language: locale.code })}
                  accessibilityState={{ selected }}
                />
              );
            })}
          </ListGroup>
        </View>

        <View>
          <SectionHeader title={t("settings.sections.behavior")} />
          <ListGroup>
            <SwitchRow
              label={t("options.autoCheckIp")}
              value={settings.behavior.autoCheckIp}
              onChange={(autoCheckIp) =>
                app.update((current) => ({
                  ...current,
                  behavior: { ...current.behavior, autoCheckIp },
                }))
              }
            />
            <SwitchRow
              last
              label={t("settings.core.logEnabled")}
              value={settings.core.logEnabled}
              onChange={(logEnabled) =>
                app.update((current) => ({
                  ...current,
                  core: { ...current.core, logEnabled },
                }))
              }
            />
          </ListGroup>
        </View>
      </> : null}
    </DetailScreen>
  );
}

/**
 * Maintenance: the rule library and the logs. The app itself is updated by
 * the store, so only the rule library is offered here — the desktop's
 * self-update has no equivalent.
 */
export function MaintenanceScreen() {
  const { language, t } = useI18n();
  const app = useAppSettings();
  const hasCoreLogs = useRuntimeEventStore((state) => state.logLines.some((line) => line.body.source === "core"));
  const ruleLibrary = useRuleLibraryUpdate();

  return (
    <DetailScreen gap="gap-6">
      {app.working ? <Spinner size="sm" /> : null}
      <ErrorNotice error={app.error} retry={app.retry} />

      <Card className="gap-4 p-5">
        <IconBadge icon={Database} />
        <View className="gap-1">
          <Typography accessibilityRole="header" maxFontSizeMultiplier={2} className="text-lg font-semibold text-foreground">
            {t("updates.ruleLibraryTitle")}
          </Typography>
          <Typography className="text-base text-subtle">{t("updates.ruleLibraryDescription")}</Typography>
        </View>
        <View className="gap-1">
          <Typography className="text-sm text-subtlest">
            {ruleLibrary.updatedAt === null
              ? t("updates.neverUpdated")
              : t("updates.lastUpdated", {
                  time: new Date(ruleLibrary.updatedAt).toLocaleString(language),
                })}
          </Typography>
          {ruleLibrary.files?.length ? (
            <Typography className="text-sm text-connected">
              {t("updates.resourceUpdated", { count: ruleLibrary.files.length })}
            </Typography>
          ) : null}
        </View>
        <ErrorNotice error={ruleLibrary.error} />
        <Button
          className="bg-accent-soft py-3"
          variant="secondary"
          isDisabled={ruleLibrary.updating}
          onPress={() => void ruleLibrary.update()}
        >
          {ruleLibrary.updating ? <Spinner size="sm" /> : null}
          <Button.Label>{t("updates.updateNow")}</Button.Label>
        </Button>
      </Card>

      <ListGroup>
        {/* The lines themselves get a screen of their own; this says whether
            the backend is delivering any, which the core-log switch under
            General decides. */}
        <ListRow
          last
          leading={<IconBadge icon={ScrollText} size="sm" tone="neutral" />}
          testID="maintenance-logs"
          chevron
          title={t("tabs.logs")}
          onPress={() => openPage("logs")}
          description={!app.settings?.core.logEnabled ? t("settings.logs.coreLogOff") : hasCoreLogs ? t("settings.logs.coreLogReceived") : t("settings.logs.coreLogWaiting")}
          descriptionLines={0}
        />
      </ListGroup>
    </DetailScreen>
  );
}
