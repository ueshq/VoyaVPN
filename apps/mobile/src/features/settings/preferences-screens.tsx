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
import { Spinner } from "heroui-native/spinner";
import { Switch } from "heroui-native/switch";
import { Typography } from "heroui-native/text";
import { Check, Database, ScrollText } from "lucide-react-native";
import { ScrollView, View, useWindowDimensions } from "react-native";

import { IconBadge } from "~/components/icon-badge";
import { ListCard } from "~/components/list-card";
import { ListRow } from "~/components/list-row";
import { SectionHeader } from "~/components/section-header";
import { SegmentedControl } from "~/components/segmented-control";
import { useToneColor } from "~/components/tone";
import { useScreenInsets } from "~/components/use-screen-insets";

/** The three theme choices, in the order the desktop offers them. */
const THEME_MODES = [
  { labelKey: "menu.themeSystem", value: "system" },
  { labelKey: "menu.themeLight", value: "light" },
  { labelKey: "menu.themeDark", value: "dark" },
] as const satisfies readonly { labelKey: TranslationKey; value: ThemeMode }[];

/**
 * Settings.
 *
 * The everyday choices only: appearance, language, and the two behaviour
 * switches that mean something on a phone. Everything the desktop keeps under
 * Advanced — the capture mode, TUN diagnostics, autostart, the close action —
 * is either a system setting here or has no meaning at all (see
 * `UNSUPPORTED_ON_MOBILE` in the mobile host).
 */
function PreferencesScreen({ section }: { section: "general" | "maintenance" }) {
  const { language, t } = useI18n();
  const insets = useScreenInsets();
  const checkColor = useToneColor("brand");
  const { width, fontScale } = useWindowDimensions();
  const stackedChoices = width / fontScale < 360;
  const app = useAppSettings();
  const appearance = app.settings?.appearance;
  const hasCoreLogs = useRuntimeEventStore((state) => state.logLines.some((line) => line.body.source === "core"));
  const ruleLibrary = useRuleLibraryUpdate();


  // Mounting this screen is what asks the backend for log lines.


  return (
    <ScrollView
      className="flex-1 bg-canvas"
      contentContainerClassName="gap-6 px-page"
      contentContainerStyle={insets}
      // The three DNS fields sit low enough that the keyboard covers them
      // completely once one is focused — measured at y=734 on a 874pt screen
      // against a keyboard whose top edge is at 538 — so without this you
      // cannot see the resolver you are editing. A plain ScrollView does not
      // inset itself for the keyboard on its own.
      automaticallyAdjustKeyboardInsets
    >


      {app.working ? <Spinner size="sm" /> : null}
      <ErrorNotice error={app.error} retry={app.retry} />

      {section === "general" && appearance ? (
        <View>
          <SectionHeader title={t("settings.sections.appearance")} />
          <Card className="gap-3 p-4">
            <Typography className="text-sm font-medium text-subtle">{t("modal.theme")}</Typography>
            <SegmentedControl
              options={THEME_MODES.map(({ labelKey, value }) => ({ label: t(labelKey), value }))}
              value={appearance.theme}
              onChange={(theme) => app.setAppearance({ ...appearance, theme })}
              stacked={stackedChoices}
            />
          </Card>
        </View>
      ) : null}

      {section === "general" && appearance ? (
        <View>
          <SectionHeader title={t("modal.language")} />
          {/* Theme and language are the same kind of choice, so they go through
              the same write: `setAppearance` previews at once and persists on
              the backend's acknowledgement. */}
          <ListCard>
            {localeOptions.map((locale, index) => {
              const selected = appearance.language === locale.code;
              return (
                <ListRow
                  key={locale.code}
                  last={index === localeOptions.length - 1}
                  title={locale.nativeName}
                  trailing={selected ? <Check size={20} color={checkColor} accessible={false} /> : null}
                  onPress={() => app.setAppearance({ ...appearance, language: locale.code })}
                  accessibilityState={{ selected }}
                />
              );
            })}
          </ListCard>
        </View>
      ) : null}

      {section === "general" && app.settings ? (
        <View className="gap-3">
          <View>
            <SectionHeader title={t("settings.sections.behavior")} />
            <ListCard>
              <Toggle
                label={t("options.autoCheckIp")}
                value={app.settings.behavior.autoCheckIp}
                onChange={(autoCheckIp) =>
                  app.update((current) => ({
                    ...current,
                    behavior: { ...current.behavior, autoCheckIp },
                  }))
                }
              />
              <Toggle
                last
                label={t("settings.core.logEnabled")}
                value={app.settings.core.logEnabled}
                onChange={(logEnabled) =>
                  app.update((current) => ({
                    ...current,
                    core: { ...current.core, logEnabled },
                  }))
                }
              />
            </ListCard>
          </View>

        </View>
      ) : null}

      {/* The app itself is updated by the store, so only the rule library is
          offered here — the desktop's self-update has no equivalent. */}
      {section === "maintenance" ? <>
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
          className="min-h-12 h-auto rounded-3xl bg-accent-soft py-3"
          variant="secondary"
          isDisabled={ruleLibrary.updating}
          onPress={() => void ruleLibrary.update()}
        >
          {ruleLibrary.updating ? <Spinner size="sm" /> : null}
          <Button.Label>{t("updates.updateNow")}</Button.Label>
        </Button>
      </Card>

      <ListCard>
        {/* The lines themselves get a screen of their own; this says whether
            the backend is delivering any, which the switch above decides. */}
        <ListRow
          last
          leading={<IconBadge icon={ScrollText} size="sm" tone="neutral" />}
          testID="maintenance-logs"
          title={t("tabs.logs")}
          onPress={() => openPage("logs")}
          description={!app.settings?.core.logEnabled ? t("settings.logs.coreLogOff") : hasCoreLogs ? t("settings.logs.coreLogReceived") : t("settings.logs.coreLogWaiting")}
          descriptionLines={0}
        />
      </ListCard>
      </> : null}
    </ScrollView>
  );
}

/** A setting that is on or off: a list row whose trailing control is the switch. */
function Toggle({
  label,
  last = false,
  onChange,
  value,
}: {
  label: string;
  last?: boolean;
  onChange: (value: boolean) => void;
  value: boolean;
}) {
  return (
    <ListRow
      last={last}
      title={label}
      trailing={
        <Switch
          isSelected={value}
          onSelectedChange={onChange}
          accessibilityLabel={label}
          hitSlop={10}
        />
      }
    />
  );
}

export function GeneralScreen() { return <PreferencesScreen section="general" />; }
export function MaintenanceScreen() { return <PreferencesScreen section="maintenance" />; }
