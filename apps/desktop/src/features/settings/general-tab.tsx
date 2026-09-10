import { useState, type KeyboardEvent } from "react";
import { Monitor, Moon, Sun } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { Input } from "@voya/ui/components/input";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationFunction, TranslationKey } from "@voya/i18n";
import type { ThemeMode } from "@/stores/preferences-store";

import {
  SettingsCheckbox,
  SettingsGroup,
  SettingsRow,
} from "./settings-form";
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

export function GeneralTab({ controller }: { controller: AppSettingsFormController }) {
  const { language, localeOptions, t } = useI18n();
  const { settings, setAppearance, update, working } = controller;
  const [emptyChord, setEmptyChord] = useState({ alt: false, control: false, shift: false, keyCode: 0 });

  const selectedLanguage = localeOptions.some((locale) => locale.code === settings.appearance.language)
    ? settings.appearance.language
    : language;
  const hotkey = settings.shortcuts.showWindowShortcut ?? emptyChord;
  function setHotkey(next: typeof hotkey) {
    if (!next.keyCode) { setEmptyChord(next); return; }
    update((current) => ({ ...current, shortcuts: { showWindowShortcut: next } }));
  }

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
                  className={cn("h-8 min-w-0 px-3", selected && selectedOptionClass)}
                  disabled={working}
                  onClick={() =>
                    setAppearance({ ...settings.appearance, theme: option.value })
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
                  className={cn("h-8 min-w-12 px-2 text-xs", selected && selectedOptionClass)}
                  disabled={working}
                  onClick={() =>
                    setAppearance({ ...settings.appearance, language: locale.code })
                  }
                  type="button"
                  variant={selected ? "secondary" : "outline"}
                >
                  {locale.label}
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

      <SettingsGroup title={t("settings.sections.shortcuts")}>
        <SettingsRow htmlFor="settings-hotkey" label={t("options.hotkeyShowWindow")} error={controller.fieldErrors["shortcuts.showWindowShortcut"]}>
          <div className="flex flex-wrap items-center gap-2">
            {(["control", "alt", "shift"] as const).map((modifier) => (
              <Button
                key={modifier}
                aria-pressed={hotkey[modifier]}
                className="h-8 px-2 text-xs"
                onClick={() => setHotkey({ ...hotkey, [modifier]: !hotkey[modifier] })}
                type="button"
                variant={hotkey[modifier] ? "secondary" : "outline"}
              >
                {modifier === "control" ? "Ctrl" : modifier[0].toUpperCase() + modifier.slice(1)}
              </Button>
            ))}
            <Input
              id="settings-hotkey"
              aria-invalid={Boolean(controller.fieldErrors["shortcuts.showWindowShortcut"]) || undefined}
              aria-describedby={controller.fieldErrors["shortcuts.showWindowShortcut"] ? "settings-hotkey-error" : undefined}
              aria-label={t("options.hotkeyKey")}
              className="h-8 w-28 px-2 text-sm"
              data-hotkey-capture=""
              placeholder={t("options.hotkeyKeyPlaceholder")}
              onKeyDown={(event) => {
                const keyCode = keyCodeFromEvent(event);
                if (keyCode !== null) {
                  setHotkey({ ...hotkey, keyCode });
                }
              }}
              readOnly
              value={keyCodeLabel(t, settings.shortcuts.showWindowShortcut?.keyCode ?? null)}
            />
            <Button
              className="h-7 px-2 text-xs"
              onClick={() => {
                setEmptyChord({ alt: false, control: false, shift: false, keyCode: 0 });
                update((current) => ({ ...current, shortcuts: { showWindowShortcut: null } }));
              }}
              type="button"
              variant="ghost"
            >
              {t("actions.clear")}
            </Button>
          </div>
        </SettingsRow>
      </SettingsGroup>
    </div>
  );
}

/**
 * Reads a shortcut key from a capture keystroke. Only keys the backend can turn
 * into an accelerator are swallowed; everything else — Tab, Escape, the bare
 * modifiers, unsupported keys — passes through so the field never traps the
 * keyboard (WCAG 2.1.2) and Escape keeps closing the surface.
 */
function keyCodeFromEvent(event: KeyboardEvent<HTMLInputElement>): number | null {
  const keyCode = HOTKEY_KEY_CODES.get(event.code);
  if (keyCode === undefined) {
    return null;
  }
  event.preventDefault();
  return keyCode;
}

/**
 * `KeyboardEvent.code` → the legacy key code persisted in
 * `shortcuts.showWindowShortcut`. Mirrors `key_code_to_accelerator_key` in
 * voya-platform so every captured key is one the backend accepts, minus `Tab`
 * (9) and `Escape` (27), which stay reserved for focus movement and cancel.
 */
function buildHotkeyKeyCodes(): ReadonlyMap<string, number> {
  const codes = new Map<string, number>(
    Object.entries({
      ArrowDown: 40,
      ArrowLeft: 37,
      ArrowRight: 39,
      ArrowUp: 38,
      Backquote: 192,
      Backslash: 220,
      Backspace: 8,
      BracketLeft: 219,
      BracketRight: 221,
      CapsLock: 20,
      Comma: 188,
      Delete: 46,
      End: 35,
      Enter: 13,
      Equal: 187,
      Home: 36,
      Insert: 45,
      Minus: 189,
      NumpadAdd: 107,
      NumpadDecimal: 110,
      NumpadDivide: 111,
      NumpadMultiply: 106,
      NumpadSubtract: 109,
      PageDown: 34,
      PageUp: 33,
      Pause: 19,
      Period: 190,
      Quote: 222,
      Semicolon: 186,
      Slash: 191,
      Space: 32,
    }),
  );
  for (let digit = 0; digit <= 9; digit += 1) {
    codes.set(`Digit${digit}`, 48 + digit);
    codes.set(`Numpad${digit}`, 96 + digit);
  }
  for (let letter = 0; letter < 26; letter += 1) {
    codes.set(`Key${String.fromCharCode(65 + letter)}`, 65 + letter);
  }
  for (let index = 1; index <= 24; index += 1) {
    codes.set(`F${index}`, 111 + index);
  }
  return codes;
}

const HOTKEY_KEY_CODES = buildHotkeyKeyCodes();

/**
 * Renders the keycap shown in the hotkey field.
 *
 * Digit, letter and function keycaps are the glyph printed on the key and stay
 * untranslated in every locale; the *named* keys ("Backspace", "Page Up", …)
 * and the unknown-code fallback are prose, so they come from
 * `options.keyName.*`. The map is rebuilt per call rather than hoisted because
 * its values are locale-dependent, and this runs once per render of a single
 * read-only input.
 */
function keyCodeLabel(t: TranslationFunction, keyCode: number | null): string {
  if (!keyCode) return "";
  if ((keyCode >= 48 && keyCode <= 57) || (keyCode >= 65 && keyCode <= 90)) {
    return String.fromCharCode(keyCode);
  }
  if (keyCode >= 112 && keyCode <= 135) return `F${keyCode - 111}`;
  return (
    {
      8: t("options.keyName.backspace"),
      9: t("options.keyName.tab"),
      13: t("options.keyName.enter"),
      27: t("options.keyName.escape"),
      32: t("options.keyName.space"),
      33: t("options.keyName.pageUp"),
      34: t("options.keyName.pageDown"),
      35: t("options.keyName.end"),
      36: t("options.keyName.home"),
      37: t("options.keyName.left"),
      38: t("options.keyName.up"),
      39: t("options.keyName.right"),
      40: t("options.keyName.down"),
      45: t("options.keyName.insert"),
      46: t("options.keyName.delete"),
    }[keyCode] ?? t("options.keyName.unknown", { keyCode })
  );
}
