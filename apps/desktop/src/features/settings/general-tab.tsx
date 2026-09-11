import { Monitor, Moon, Sun } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import type { ThemeMode } from "@/stores/preferences-store";

import { SettingsCheckbox, SettingsGroup, SettingsRow } from "./settings-form";
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
      </SettingsGroup>
    </div>
  );
}
