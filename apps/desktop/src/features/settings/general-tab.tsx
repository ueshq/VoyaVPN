import { Monitor, Moon, Sun } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import type { CloseAction } from "@/ipc/bindings";
import type { ThemeMode } from "@/stores/preferences-store";

import { SelectField, SettingsCheckbox, SettingsGroup, SettingsRow } from "./settings-form";
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

const selectedOptionClass =
  "border border-primary bg-accent-blue-light text-brand hover:bg-accent-blue-light hover:text-brand";

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
        <SettingsRow label={t("modal.theme")}>
          <div className="flex flex-wrap gap-2">
            {themeOptions.map((option) => {
              const Icon = option.icon;
              const selected = settings.appearance.theme === option.value;
              return (
                <Button
                  key={option.value}
                  aria-pressed={selected}
                  className={cn(
                    "h-8 min-w-0 px-3",
                    selected && selectedOptionClass,
                  )}
                  disabled={working}
                  onClick={() =>
                    setAppearance({
                      ...settings.appearance,
                      theme: option.value,
                    })
                  }
                  type="button"
                  variant={selected ? "secondary" : "outline"}
                >
                  <Icon className="size-4" aria-hidden="true" />
                  <span className="truncate">{t(option.labelKey)}</span>
                </Button>
              );
            })}
          </div>
        </SettingsRow>

        <SettingsRow label={t("modal.language")}>
          <div className="flex flex-wrap gap-2">
            {localeOptions.map((locale) => {
              const selected = selectedLanguage === locale.code;
              return (
                <Button
                  key={locale.code}
                  aria-pressed={selected}
                  className={cn(
                    "h-8 min-w-12 px-2 text-xs",
                    selected && selectedOptionClass,
                  )}
                  disabled={working}
                  onClick={() =>
                    setAppearance({
                      ...settings.appearance,
                      language: locale.code,
                    })
                  }
                  type="button"
                  variant={selected ? "secondary" : "outline"}
                >
                  {locale.nativeName}
                </Button>
              );
            })}
          </div>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup title={t("settings.startup")}>
        <SettingsCheckbox
          field="behavior.autostart"
          checked={settings.behavior.autostart}
          disabled={working}
          label={t("options.autostart")}
          onCheckedChange={(checked) =>
            update((current) => ({
              ...current,
              behavior: { ...current.behavior, autostart: checked === true },
            }))
          }
        />
        <SettingsCheckbox
          field="behavior.startMinimized"
          checked={settings.behavior.startMinimized}
          description={t("options.startMinimizedHint")}
          disabled={working}
          label={t("options.startMinimized")}
          onCheckedChange={(checked) =>
            update((current) => ({
              ...current,
              behavior: { ...current.behavior, startMinimized: checked === true },
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
          optionLabel={(action) => t(CLOSE_ACTION_LABELS[action as CloseAction])}
          options={CLOSE_ACTIONS}
          value={settings.behavior.closeAction}
        />
      </SettingsGroup>

      <SettingsGroup title={t("settings.sections.connection")}>
        <SettingsCheckbox
          field="behavior.autoCheckIp"
          checked={settings.behavior.autoCheckIp}
          disabled={working}
          label={t("options.autoCheckIp")}
          onCheckedChange={(checked) =>
            update((current) => ({
              ...current,
              behavior: { ...current.behavior, autoCheckIp: checked === true },
            }))
          }
        />
        <SettingsCheckbox
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
                autoCreateSubscriptionGroup: checked === true,
              },
            }))
          }
        />
      </SettingsGroup>
    </div>
  );
}
