import type { TranslationKey } from "@voya/i18n/core";

/**
 * The bottom tabs.
 *
 * Deliberately a subset of the desktop's six sections: `selfHost` is dropped
 * because a phone is not an exit node, and the labels reuse the desktop's
 * `tabs.*` keys so both shells name the same section the same way.
 */
export const SHELL_TABS = {
  home: { titleKey: "tabs.home" },
  profiles: { titleKey: "tabs.profiles" },
  rules: { titleKey: "tabs.rules" },
  settings: { titleKey: "tabs.settings" },
} as const satisfies Record<string, { titleKey: TranslationKey }>;

export type ShellTab = keyof typeof SHELL_TABS;
