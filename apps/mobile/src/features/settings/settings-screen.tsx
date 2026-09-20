import { useAppSettings } from "@voya/features/settings/use-app-settings";
import { useDnsSettings } from "@voya/features/dns/use-dns-settings";
import { useLogStream } from "@voya/features/logs/use-log-stream";
import { useRuleLibraryUpdate } from "@voya/features/updates/use-rule-library-update";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { localeOptions } from "@voya/i18n/core";
import { useI18n } from "@voya/i18n/use-i18n";
import type { ThemeMode } from "@voya/contracts";
import type { TranslationKey } from "@voya/i18n/core";
import { ScrollView, Switch, View } from "react-native";

import { Button, ButtonSpinner, ButtonText } from "~/components/ui/button";
import { Card } from "~/components/ui/card";
import { Input, InputField } from "~/components/ui/input";
import { Pressable } from "~/components/ui/pressable";
import { Text } from "~/components/ui/text";

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
  const app = useAppSettings();
  const appearance = app.settings?.appearance;
  const logLines = useRuntimeEventStore((state) => state.logLines.length);
  const ruleLibrary = useRuleLibraryUpdate();
  const dns = useDnsSettings(true);

  // Mounting this screen is what asks the backend for log lines.
  useLogStream();

  return (
    <ScrollView className="flex-1 bg-canvas" contentContainerClassName="gap-4 p-page">
      {appearance ? (
        <Card className="gap-3 rounded-card bg-card p-4">
          <Text className="text-section text-foreground">{t("modal.theme")}</Text>
          <View className="flex-row gap-2">
            {THEME_MODES.map(({ labelKey, value }) => (
              <Pressable
                key={value}
                className={`flex-1 items-center rounded-control border px-3 py-2 ${
                  appearance.theme === value
                    ? "border-brand bg-brand-tint"
                    : "border-border bg-surface"
                }`}
                onPress={() => app.setAppearance({ ...appearance, theme: value })}
                accessibilityRole="button"
                accessibilityState={{ selected: appearance.theme === value }}
              >
                <Text className="text-caption text-foreground">{t(labelKey)}</Text>
              </Pressable>
            ))}
          </View>
        </Card>
      ) : null}

      {appearance ? (
        <Card className="gap-3 rounded-card bg-card p-4">
          <Text className="text-section text-foreground">{t("modal.language")}</Text>
          {/* Theme and language are the same kind of choice, so they go through
              the same write: `setAppearance` previews at once and persists on
              the backend's acknowledgement. */}
          {localeOptions.map((locale) => (
            <Pressable
              key={locale.code}
              className="flex-row items-center justify-between py-2"
              onPress={() => app.setAppearance({ ...appearance, language: locale.code })}
              accessibilityRole="button"
              accessibilityState={{ selected: appearance.language === locale.code }}
            >
              <Text className="text-body text-foreground">{locale.nativeName}</Text>
              {appearance.language === locale.code ? (
                <Text className="text-caption text-brand">{t("panes.profiles.card.default")}</Text>
              ) : null}
            </Pressable>
          ))}
        </Card>
      ) : null}

      {app.settings ? (
        <Card className="gap-3 rounded-card bg-card p-4">
          <Text className="text-section text-foreground">{t("settings.sections.behavior")}</Text>
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
            label={t("settings.core.logEnabled")}
            value={app.settings.core.logEnabled}
            onChange={(logEnabled) =>
              app.update((current) => ({
                ...current,
                core: { ...current.core, logEnabled },
              }))
            }
          />
          {app.error ? <Text className="text-caption text-danger">{app.error}</Text> : null}
        </Card>
      ) : null}

      {/* The three resolvers and the two switches that change what gets
          resolved at all. The rest of the desktop's DNS pane — strategies,
          expected IPs, Hosts — is a text-editing job a phone should not ask
          for; the desktop stays the place to do it, and what is set there is
          kept as it is. */}
      {dns.form ? (
        <Card className="gap-3 rounded-card bg-card p-4">
          <Text className="text-section text-foreground">{t("panes.dns.title")}</Text>
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
          <Toggle
            label={t("panes.dns.fakeIp")}
            value={dns.form.fakeIp ?? false}
            onChange={(fakeIp) => dns.updateSimple({ fakeIp })}
          />
          <Toggle
            label={t("panes.dns.blockBindingQuery")}
            value={dns.form.blockBindingQuery ?? false}
            onChange={(blockBindingQuery) => dns.updateSimple({ blockBindingQuery })}
          />
          {dns.operationError ? (
            <Text className="text-caption text-danger">{dns.operationError}</Text>
          ) : null}
        </Card>
      ) : null}

      {/* The app itself is updated by the store, so only the rule library is
          offered here — the desktop's self-update has no equivalent. */}
      <Card className="gap-3 rounded-card bg-card p-4">
        <Text className="text-section text-foreground">{t("updates.ruleLibraryTitle")}</Text>
        <Text className="text-caption text-subtle">{t("updates.ruleLibraryDescription")}</Text>
        <Text className="text-caption text-subtlest">
          {ruleLibrary.updatedAt === null
            ? t("updates.neverUpdated")
            : t("updates.lastUpdated", {
                time: new Date(ruleLibrary.updatedAt).toLocaleString(language),
              })}
        </Text>
        {ruleLibrary.files?.length ? (
          <Text className="text-caption text-subtle">
            {t("updates.resourceUpdated", { count: ruleLibrary.files.length })}
          </Text>
        ) : null}
        {ruleLibrary.error ? (
          <Text className="text-caption text-danger">{ruleLibrary.error}</Text>
        ) : null}
        <Button
          size="sm"
          variant="outline"
          isDisabled={ruleLibrary.updating}
          onPress={() => void ruleLibrary.update()}
        >
          {ruleLibrary.updating ? <ButtonSpinner /> : null}
          <ButtonText>{t("updates.updateNow")}</ButtonText>
        </Button>
      </Card>

      <Card className="gap-1 rounded-card bg-card p-4">
        <Text className="text-section text-foreground">{t("tabs.logs")}</Text>
        {/* The lines themselves get a screen of their own; this says whether
            the backend is delivering any, which the switch above decides. */}
        <Text className="text-caption text-subtle">
          {logLines > 0 ? t("proxy.monitorLive") : t("settings.logs.coreLogOff")}
        </Text>
      </Card>
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
    <View className="gap-1">
      <Text className="text-caption text-subtle">{label}</Text>
      <Input>
        <InputField
          value={value ?? ""}
          onChangeText={onChange}
          autoCapitalize="none"
          autoCorrect={false}
          accessibilityLabel={label}
        />
      </Input>
      {error ? <Text className="text-caption text-danger">{error}</Text> : null}
    </View>
  );
}

function Toggle({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: boolean) => void;
  value: boolean;
}) {
  return (
    <View className="flex-row items-center justify-between">
      <Text className="flex-1 pr-3 text-body text-foreground">{label}</Text>
      <Switch value={value} onValueChange={onChange} accessibilityLabel={label} />
    </View>
  );
}
