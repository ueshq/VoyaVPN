import type { TranslationKey } from "@voya/i18n/core";
import type { AppleIcon } from "react-native-bottom-tabs";
import { Platform, type ImageSourcePropType } from "react-native";
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
 *
 * Android uses density-qualified PNG drawables (SVG masters in assets/tab-icons).
 * iOS resolves SF Symbols itself. Both platforms tint the icons natively.
 */
export const SHELL_TABS = {
  home: {
    titleKey: "tabs.home",
    icon: Platform.OS === "ios"
      ? { sfSymbol: "house" }
      : { uri: "voya_tab_home" },
    component: HomeScreen,
  },
  profiles: {
    titleKey: "tabs.profiles",
    icon: Platform.OS === "ios"
      ? { sfSymbol: "server.rack" }
      : { uri: "voya_tab_nodes" },
    component: NodesScreen,
  },
  rules: {
    titleKey: "tabs.rules",
    icon: Platform.OS === "ios"
      ? { sfSymbol: "point.3.connected.trianglepath.dotted" }
      : { uri: "voya_tab_rules" },
    component: RulesScreen,
  },
  settings: {
    titleKey: "tabs.settings",
    icon: Platform.OS === "ios"
      ? { sfSymbol: "gearshape" }
      : { uri: "voya_tab_settings" },
    component: SettingsScreen,
  },
} as const satisfies Record<string, { titleKey: TranslationKey; icon: AppleIcon | ImageSourcePropType; component: ComponentType }>;

export type ShellTab = keyof typeof SHELL_TABS;
