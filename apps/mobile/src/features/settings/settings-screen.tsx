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
import { PressableFeedback } from "heroui-native/pressable-feedback";
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { ScrollView, Switch, View } from "react-native";

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
    <ScrollView
      className="flex-1 bg-canvas"
      contentContainerClassName="gap-4 p-page"
      // The three DNS fields sit low enough that the keyboard covers them
      // completely once one is focused — measured at y=734 on a 874pt screen
      // against a keyboard whose top edge is at 538 — so without this you
      // cannot see the resolver you are editing. A plain ScrollView does not
      // inset itself for the keyboard on its own.
      automaticallyAdjustKeyboardInsets
    >
      {appearance ? (
        <Card className="gap-3">
          <Typography className="text-section text-foreground">{t("modal.theme")}</Typography>
          <View className="flex-row gap-2">
            {THEME_MODES.map(({ labelKey, value }) => (
              <PressableFeedback
                key={value}
                className={`flex-1 items-center rounded-2xl border px-3 py-2 ${
                  appearance.theme === value
                    ? "border-brand bg-brand-tint"
                    : "border-border bg-surface"
                }`}
                onPress={() => app.setAppearance({ ...appearance, theme: value })}
                accessibilityRole="button"
                accessibilityState={{ selected: appearance.theme === value }}
              >
                <PressableFeedback.Highlight />
                <Typography className="text-caption text-foreground">{t(labelKey)}</Typography>
              </PressableFeedback>
            ))}
          </View>
        </Card>
      ) : null}

      {appearance ? (
        <Card className="gap-3">
          <Typography className="text-section text-foreground">{t("modal.language")}</Typography>
          {/* Theme and language are the same kind of choice, so they go through
              the same write: `setAppearance` previews at once and persists on
              the backend's acknowledgement. */}
          {localeOptions.map((locale) => (
            <PressableFeedback
              key={locale.code}
              animation="disable-all"
              className="flex-row items-center justify-between py-2"
              onPress={() => app.setAppearance({ ...appearance, language: locale.code })}
              accessibilityRole="button"
              accessibilityState={{ selected: appearance.language === locale.code }}
            >
              <PressableFeedback.Highlight />
              <Typography className="text-body text-foreground">{locale.nativeName}</Typography>
              {appearance.language === locale.code ? (
                <Typography className="text-caption text-brand">
                  {t("panes.profiles.card.default")}
                </Typography>
              ) : null}
            </PressableFeedback>
          ))}
        </Card>
      ) : null}

      {app.settings ? (
        <Card className="gap-3">
          <Typography className="text-section text-foreground">
            {t("settings.sections.behavior")}
          </Typography>
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
          {app.error ? <Typography className="text-caption text-danger">{app.error}</Typography> : null}
        </Card>
      ) : null}

      {/* The three resolvers and the two switches that change what gets
          resolved at all. The rest of the desktop's DNS pane — strategies,
          expected IPs, Hosts — is a text-editing job a phone should not ask
          for; the desktop stays the place to do it, and what is set there is
          kept as it is. */}
      {dns.form ? (
        <Card className="gap-3">
          <Typography className="text-section text-foreground">{t("panes.dns.title")}</Typography>
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
            <Typography className="text-caption text-danger">{dns.operationError}</Typography>
          ) : null}
        </Card>
      ) : null}

      {/* The app itself is updated by the store, so only the rule library is
          offered here — the desktop's self-update has no equivalent. */}
      <Card className="gap-3">
        <Typography className="text-section text-foreground">
          {t("updates.ruleLibraryTitle")}
        </Typography>
        <Typography className="text-caption text-subtle">
          {t("updates.ruleLibraryDescription")}
        </Typography>
        <Typography className="text-caption text-subtlest">
          {ruleLibrary.updatedAt === null
            ? t("updates.neverUpdated")
            : t("updates.lastUpdated", {
                time: new Date(ruleLibrary.updatedAt).toLocaleString(language),
              })}
        </Typography>
        {ruleLibrary.files?.length ? (
          <Typography className="text-caption text-subtle">
            {t("updates.resourceUpdated", { count: ruleLibrary.files.length })}
          </Typography>
        ) : null}
        {ruleLibrary.error ? (
          <Typography className="text-caption text-danger">{ruleLibrary.error}</Typography>
        ) : null}
        <Button
          size="sm"
          variant="outline"
          isDisabled={ruleLibrary.updating}
          onPress={() => void ruleLibrary.update()}
        >
          {ruleLibrary.updating ? <Spinner size="sm" /> : null}
          <Button.Label>{t("updates.updateNow")}</Button.Label>
        </Button>
      </Card>

      <Card className="gap-1">
        <Typography className="text-section text-foreground">{t("tabs.logs")}</Typography>
        {/* The lines themselves get a screen of their own; this says whether
            the backend is delivering any, which the switch above decides. */}
        <Typography className="text-caption text-subtle">
          {logLines > 0 ? t("proxy.monitorLive") : t("settings.logs.coreLogOff")}
        </Typography>
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
      <Typography className="text-caption text-subtle">{label}</Typography>
      <Input
        value={value ?? ""}
        onChangeText={onChange}
        autoCapitalize="none"
        autoCorrect={false}
        accessibilityLabel={label}
        isInvalid={Boolean(error)}
      />
      {error ? <Typography className="text-caption text-danger">{error}</Typography> : null}
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
      <Typography className="flex-1 pr-3 text-body text-foreground">{label}</Typography>
      <Switch value={value} onValueChange={onChange} accessibilityLabel={label} />
    </View>
  );
}
