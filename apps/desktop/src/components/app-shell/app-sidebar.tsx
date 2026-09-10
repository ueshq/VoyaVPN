import type * as React from "react";
import { Activity, ArrowDown, ArrowUp, Home, PanelLeft, Route, Settings, Shield } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { SidebarNavItem } from "@/components/app-shell/sidebar-nav-item";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import { useRuntimeEventStore } from "@/ipc";
import type { CoreState, TitleBarLayout } from "@/ipc/bindings";
import { formatBytesPerSecond } from "@voya/utils/formatting";
import { type ShellTab, useShellStore } from "@/stores/shell-store";

// `id` of the content `tabpanel` the nav controls. Exported so the shell can tag
// the panel element with a matching id for `aria-controls` / `aria-labelledby`.
export const SHELL_PANEL_ID = "shell-tabpanel";

type NavItem = { icon: LucideIcon; titleKey: TranslationKey; value: ShellTab };

// Keep every existing destination reachable in both sidebar widths.
const navItems: NavItem[] = [
  { icon: Home, titleKey: "tabs.home", value: "home" },
  { icon: Shield, titleKey: "tabs.profiles", value: "profiles" },
  { icon: Route, titleKey: "tabs.rules", value: "rules" },
  { icon: Activity, titleKey: "tabs.connections", value: "connections" },
  { icon: Settings, titleKey: "tabs.settings", value: "settings" },
];

const CORE_STATE_TRANSLATION_KEYS = {
  cleanupPending: "home.cleanupPending",
  connected: "status.connected",
  connecting: "status.connecting",
  disconnected: "status.disconnected",
  disconnecting: "status.disconnecting",
} as const satisfies Record<CoreState, TranslationKey>;

export function AppSidebar({ titleBarLayout }: { titleBarLayout: TitleBarLayout }) {
  const { t } = useI18n();
  const activeTab = useShellStore((state) => state.activeTab);
  const setActiveTab = useShellStore((state) => state.setActiveTab);
  const collapsed = useShellStore((state) => state.sidebarCollapsed);
  const toggleSidebar = useShellStore((state) => state.toggleSidebar);

  // Manual-activation tablist keyboard pattern: arrows move focus between the
  // roving-tabIndex tabs (Enter/Space then activates the native button). The
  // roving tabIndex alone made inactive tabs unreachable by keyboard.
  function handleNavKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    const keys = ["ArrowDown", "ArrowUp", "Home", "End"];
    if (!keys.includes(event.key)) {
      return;
    }
    const tabs = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('[role="tab"]'));
    if (tabs.length === 0) {
      return;
    }
    const current = tabs.indexOf(document.activeElement as HTMLElement);
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? tabs.length - 1
          : event.key === "ArrowDown"
            ? (current + 1 + tabs.length) % tabs.length
            : (current - 1 + tabs.length) % tabs.length;
    event.preventDefault();
    tabs[next]?.focus();
  }

  return (
    <aside className="app-sidebar" data-collapsed={collapsed}>
      <div className="sidebar-toolbar" data-tauri-drag-region={titleBarLayout !== "none" ? true : undefined}>
        <button
          aria-expanded={!collapsed}
          aria-controls="sidebar-navigation"
          aria-label={t(collapsed ? "sidebar.expand" : "sidebar.collapse")}
          className="sidebar-toggle"
          onClick={toggleSidebar}
          type="button"
        >
          <PanelLeft aria-hidden="true" className="size-4" />
        </button>
      </div>

      <nav
        aria-label={t("tabs.aria")}
        aria-orientation="vertical"
        className="sidebar-navigation"
        id="sidebar-navigation"
        onKeyDown={handleNavKeyDown}
        role="tablist"
      >
        {navItems.map((item) => (
          <SidebarNavItem
            key={item.value}
            active={activeTab === item.value}
            collapsed={collapsed}
            icon={item.icon}
            id={`shell-tab-${item.value}`}
            label={t(item.titleKey)}
            onSelect={() => setActiveTab(item.value)}
            panelId={SHELL_PANEL_ID}
          />
        ))}
      </nav>

      <SidebarFooter />
    </aside>
  );
}

// Always-mounted footer with the live connection state and up/down rates,
// absorbing what the removed bottom status bar used to show. Speeds read the
// statistics transient stream and render 0 B/s while it is quiet or disabled.
function SidebarFooter() {
  const { t } = useI18n();
  const coreState = useRuntimeEventStore((state) => state.coreState);
  const statistics = useRuntimeEventStore((state) => state.statistics);

  const state = coreState?.state ?? "disconnected";
  const upload = formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0);
  const download = formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0);

  return (
    <div aria-label={t("status.aria")} className="sidebar-footer" data-testid="sidebar-footer">
      <span className="sr-only">{t(CORE_STATE_TRANSLATION_KEYS[state])}</span>
      <p className="sidebar-traffic-title">{t("sidebar.traffic")}</p>
      <div className="sidebar-traffic-row" aria-label={t("status.upload", { speed: upload })}>
        <ArrowUp aria-hidden="true" className="size-4" />
        <span className="sidebar-traffic-label">{t("sidebar.upload")}{" "}</span>
        <span className="sidebar-traffic-value">{upload}</span>
      </div>
      <div className="sidebar-traffic-row" aria-label={t("status.download", { speed: download })}>
        <ArrowDown aria-hidden="true" className="size-4" />
        <span className="sidebar-traffic-label">{t("sidebar.download")}{" "}</span>
        <span className="sidebar-traffic-value">{download}</span>
      </div>
    </div>
  );
}
