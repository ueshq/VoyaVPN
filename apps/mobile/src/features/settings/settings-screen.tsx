import { useAppSettings } from "@voya/features/settings/use-app-settings";
import { useDnsSettings } from "@voya/features/dns/use-dns-settings";
import { useLogStream } from "@voya/features/logs/use-log-stream";
import { useRuleLibraryUpdate } from "@voya/features/updates/use-rule-library-update";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { localeOptions } from "@voya/i18n/core";
import { useI18n } from "@voya/i18n/use-i18n";
import type { ThemeMode } from "@voya/contracts";
import type { TranslationKey } from "@voya/i18n/core";
import { Button } from "heroui-native/button";
import { Card } from "heroui-native/card";
import { Input } from "heroui-native/input";
import { Spinner } from "heroui-native/spinner";
import { Switch } from "heroui-native/switch";
import { Typography } from "heroui-native/text";
import { Check, Database, ScrollText } from "lucide-react-native";
import { ScrollView, View, useWindowDimensions } from "react-native";

import { Banner } from "~/components/banner";
import { IconBadge } from "~/components/icon-badge";
import { ListCard } from "~/components/list-card";
import { ListRow } from "~/components/list-row";
import { PageHeader } from "~/components/page-header";
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
export function SettingsScreen() {
  const { language, t } = useI18n();
  const insets = useScreenInsets();
  const checkColor = useToneColor("brand");
  const { width, fontScale } = useWindowDimensions();
  const stackedChoices = width / fontScale < 360;
  const app = useAppSettings();
  const appearance = app.settings?.appearance;
  const hasCoreLogs = useRuntimeEventStore((state) => state.logLines.some((line) => line.body.source === "core"));
  const ruleLibrary = useRuleLibraryUpdate();
  const dns = useDnsSettings(true);

  // Mounting this screen is what asks the backend for log lines.
  useLogStream();

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
      <PageHeader title={t("tabs.settings")} />

      {appearance ? (
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

      {appearance ? (
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

      {app.settings ? (
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
          {app.error ? <Banner status="danger" message={app.error} /> : null}
        </View>
      ) : null}

      {/* The three resolvers and the two switches that change what gets
          resolved at all. The rest of the desktop's DNS pane — strategies,
          expected IPs, Hosts — is a text-editing job a phone should not ask
          for; the desktop stays the place to do it, and what is set there is
          kept as it is. */}
      {dns.form ? (
        <View className="gap-3">
          <View>
            <SectionHeader title={t("panes.dns.title")} />
            <Card className="gap-4 p-4">
              <DnsField
                label={t("panes.dns.remoteDns")}
                value={dns.form.remote}
                error={dns.fieldErrors.remote}
                onChange={(remote) => dns.updateSimple({ remote })}
              />
              <DnsField
                label={t("panes.dns.directDns")}
                value={dns.form.direct}
                error={dns.fieldErrors.direct}
                onChange={(direct) => dns.updateSimple({ direct })}
              />
              <DnsField
                label={t("panes.dns.bootstrapDns")}
                value={dns.form.bootstrap}
                error={dns.fieldErrors.bootstrap}
                onChange={(bootstrap) => dns.updateSimple({ bootstrap })}
              />
            </Card>
          </View>
          <ListCard>
            <Toggle
              label={t("panes.dns.fakeIp")}
              value={dns.form.fakeIp ?? false}
              onChange={(fakeIp) => dns.updateSimple({ fakeIp })}
            />
            <Toggle
              last
              label={t("panes.dns.blockBindingQuery")}
              value={dns.form.blockBindingQuery ?? false}
              onChange={(blockBindingQuery) => dns.updateSimple({ blockBindingQuery })}
            />
          </ListCard>
          {dns.operationError ? <Banner status="danger" message={dns.operationError} /> : null}
        </View>
      ) : null}

      {/* The app itself is updated by the store, so only the rule library is
          offered here — the desktop's self-update has no equivalent. */}
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
        {ruleLibrary.error ? <Banner status="danger" message={ruleLibrary.error} /> : null}
        <Button
          className="min-h-12 h-auto rounded-full bg-accent-soft py-3"
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
          title={t("tabs.logs")}
          description={!app.settings?.core.logEnabled ? t("settings.logs.coreLogOff") : hasCoreLogs ? t("settings.logs.coreLogReceived") : t("settings.logs.coreLogWaiting")}
          descriptionLines={0}
        />
      </ListCard>
    </ScrollView>
  );
}

/**
 * One resolver address.
 *
 * Saved as it is typed, like every other setting here: the shared draft
 * debounces and validates, and a rejected value keeps what was typed so it can
 * be corrected rather than retyped.
 */
function DnsField({
  error,
  label,
  onChange,
  value,
}: {
  error: string | undefined;
  label: string;
  onChange: (value: string) => void;
  value: string | null;
}) {
  return (
    <View className="gap-1.5">
      <Typography className="text-sm font-medium text-subtle">{label}</Typography>
      <Input
        variant="secondary"
        className="min-h-12 h-auto py-3"
        value={value ?? ""}
        onChangeText={onChange}
        autoCapitalize="none"
        autoCorrect={false}
        accessibilityLabel={label}
        isInvalid={Boolean(error)}
      />
      {error ? <Typography className="text-sm text-danger">{error}</Typography> : null}
    </View>
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
