import type { TranslationKey } from "@voya/i18n/core";
import { House, Route, Server, Settings, type LucideIcon } from "lucide-react-native";
import type { ComponentType } from "react";

import { HomeScreen } from "~/features/home/home-screen";
import { NodesScreen } from "~/features/profiles/nodes-screen";
import { RulesScreen } from "~/features/routing/rules-screen";
import { SettingsScreen } from "~/features/settings/settings-screen";

/**
 * The bottom tabs: each one's title, icon and screen, in bar order.
 *
 * Deliberately a subset of the desktop's six sections: `selfHost` is dropped
 * because a phone is not an exit node, and the titles reuse the desktop's
 * `tabs.*` keys so both shells name the same section the same way.
 */
export const SHELL_TABS = {
  home: { titleKey: "tabs.home", icon: House, component: HomeScreen },
  profiles: { titleKey: "tabs.profiles", icon: Server, component: NodesScreen },
  rules: { titleKey: "tabs.rules", icon: Route, component: RulesScreen },
  settings: { titleKey: "tabs.settings", icon: Settings, component: SettingsScreen },
} as const satisfies Record<string, { titleKey: TranslationKey; icon: LucideIcon; component: ComponentType }>;

export type ShellTab = keyof typeof SHELL_TABS;
