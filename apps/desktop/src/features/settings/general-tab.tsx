import { Monitor, Moon, Sun } from "lucide-react";

import { SegmentedControl, SegmentedControlItem } from "@voya/ui/components/segmented-control";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import type { CloseAction } from "@/ipc/bindings";
import type { ThemeMode } from "@/stores/preferences-store";

import { SelectField, SettingsGroup, SettingsRow, SettingsSwitch } from "./settings-form";
import type { AppSettingsFormController } from "./use-app-settings";

const themeOptions: Array<{
  icon: typeof Monitor;
  labelKey: TranslationKey;
  value: ThemeMode;
}> = [
  { icon: Monitor, labelKey: "menu.themeSystem", value: "system" },
  { icon: Sun, labelKey: "menu.themeLight", value: "light" },
  { icon: Moon, labelKey: "menu.themeDark", value: "dark" },
];

const CLOSE_ACTIONS = ["minimizeToTray", "ask", "quit"] as const satisfies readonly CloseAction[];
const CLOSE_ACTION_LABELS: Record<CloseAction, TranslationKey> = {
  ask: "settings.closeActionOptions.ask",
  minimizeToTray: "settings.closeActionOptions.minimizeToTray",
  quit: "settings.closeActionOptions.quit",
};

export function GeneralTab({
  controller,
}: {
  controller: AppSettingsFormController;
}) {
  const { language, localeOptions, t } = useI18n();
  const { settings, setAppearance, update, working } = controller;

  const selectedLanguage = localeOptions.some(
    (locale) => locale.code === settings.appearance.language,
  )
    ? settings.appearance.language
    : language;

  return (
    <div className="grid gap-4">
      <SettingsGroup title={t("settings.sections.appearance")}>
        {/* Theme and language are the same kind of choice, so they are the same control. */}
        <SettingsRow label={t("modal.theme")}>
          <SegmentedControl>
            {themeOptions.map((option) => {
              const Icon = option.icon;
              return (
                <SegmentedControlItem
                  key={option.value}
                  disabled={working}
                  onClick={() =>
                    setAppearance({
                      ...settings.appearance,
                      theme: option.value,
                    })
                  }
                  pressed={settings.appearance.theme === option.value}
                >
                  <Icon className="size-4" aria-hidden="true" />
                  <span className="truncate">{t(option.labelKey)}</span>
                </SegmentedControlItem>
              );
            })}
          </SegmentedControl>
        </SettingsRow>

        <SettingsRow label={t("modal.language")}>
          <SegmentedControl>
            {localeOptions.map((locale) => (
              <SegmentedControlItem
                key={locale.code}
                disabled={working}
                onClick={() =>
                  setAppearance({
                    ...settings.appearance,
                    language: locale.code,
                  })
                }
                pressed={selectedLanguage === locale.code}
              >
                {locale.nativeName}
              </SegmentedControlItem>
            ))}
          </SegmentedControl>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title={t("settings.startup")}>
        <SettingsSwitch
          field="behavior.autostart"
          checked={settings.behavior.autostart}
          disabled={working}
          label={t("options.autostart")}
          onCheckedChange={(checked) =>
            update((current) => ({
              ...current,
              behavior: { ...current.behavior, autostart: checked },
            }))
          }
        />
        <SettingsSwitch
          field="behavior.startMinimized"
          checked={settings.behavior.startMinimized}
          description={t(
            settings.behavior.autostart
              ? "options.startMinimizedHint"
              : "options.startMinimizedNeedsAutostart",
          )}
          disabled={working || !settings.behavior.autostart}
          label={t("options.startMinimized")}
          onCheckedChange={(checked) =>
            update((current) => ({
              ...current,
              behavior: { ...current.behavior, startMinimized: checked },
            }))
          }
        />
      </SettingsGroup>

      <SettingsGroup title={t("settings.sections.window")}>
        <SelectField
          disabled={working}
          field="behavior.closeAction"
          id="rt-close-action"
          label={t("settings.closeAction")}
          onChange={(closeAction) =>
            update((current) => ({
              ...current,
              behavior: { ...current.behavior, closeAction: closeAction as CloseAction },
            }))
          }
          optionLabel={CLOSE_ACTION_LABELS}
          options={CLOSE_ACTIONS}
          value={settings.behavior.closeAction}
        />
      </SettingsGroup>

      <SettingsGroup title={t("settings.sections.behavior")}>
        <SettingsSwitch
          field="behavior.autoCheckIp"
          checked={settings.behavior.autoCheckIp}
          disabled={working}
          label={t("options.autoCheckIp")}
          onCheckedChange={(checked) =>
            update((current) => ({
              ...current,
              behavior: { ...current.behavior, autoCheckIp: checked },
            }))
          }
        />
        <SettingsSwitch
          field="behavior.autoCreateSubscriptionGroup"
          checked={settings.behavior.autoCreateSubscriptionGroup}
          description={t("options.autoCreateSubscriptionGroupHint")}
          disabled={working}
          label={t("options.autoCreateSubscriptionGroup")}
          onCheckedChange={(checked) =>
            update((current) => ({
              ...current,
              behavior: {
                ...current.behavior,
                autoCreateSubscriptionGroup: checked,
              },
            }))
          }
        />
      </SettingsGroup>
    </div>
  );
}
