import { Database, Home, Network, Plug, Route, ScrollText, Shield } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { BrandMark } from "@/assets/brand-mark";
import { SidebarFooter, type RegionalPresetOption } from "@/components/app-shell/sidebar-footer";
import { SidebarNavItem } from "@/components/app-shell/sidebar-nav-item";
import { SidebarSectionHeader } from "@/components/app-shell/sidebar-section-header";
import { useI18n } from "@/i18n/use-i18n";
import { type ShellTab, useShellStore } from "@/stores/shell-store";

// `id` of the content `tabpanel` the nav controls. Exported so the shell can tag
// the panel element with a matching id for `aria-controls` / `aria-labelledby`.
export const SHELL_PANEL_ID = "shell-tabpanel";

type NavItem = { icon: LucideIcon; titleKey: string; value: ShellTab };

// The 7 destinations split into the always-on primary cluster and a collapsible
// NETWORK section. Labels reuse the existing `tabs.*` keys; only the section
// heading (`tabs.network`) is new.
const primaryNav: NavItem[] = [
  { icon: Home, titleKey: "tabs.home", value: "home" },
  { icon: Shield, titleKey: "tabs.profiles", value: "profiles" },
];

const networkNav: NavItem[] = [
  { icon: Route, titleKey: "tabs.routing", value: "routing" },
  { icon: Database, titleKey: "tabs.dns", value: "dns" },
  { icon: Network, titleKey: "tabs.clashProxies", value: "clash-proxies" },
  { icon: Plug, titleKey: "tabs.clashConnections", value: "clash-connections" },
  { icon: ScrollText, titleKey: "tabs.logs", value: "logs" },
];

export function AppSidebar({
  onSelectPreset,
}: {
  onSelectPreset: (option: RegionalPresetOption) => void;
}) {
  const { t } = useI18n();
  const activeTab = useShellStore((state) => state.activeTab);
  const setActiveTab = useShellStore((state) => state.setActiveTab);

  function renderNavItem(item: NavItem) {
    return (
      <SidebarNavItem
        key={item.value}
        active={activeTab === item.value}
        icon={item.icon}
        id={`shell-tab-${item.value}`}
        label={t(item.titleKey)}
        onSelect={() => setActiveTab(item.value)}
        panelId={SHELL_PANEL_ID}
      />
    );
  }

  return (
    <aside className="flex h-full min-h-0 w-60 flex-col border-e border-sidebar-border bg-sidebar text-sidebar-foreground">
      <div className="flex h-12 shrink-0 items-center gap-3 px-4">
        <BrandMark className="size-8 shrink-0 rounded-lg" aria-hidden="true" />
        <h1 className="truncate text-sm font-semibold leading-none">{t("app.name")}</h1>
      </div>

      <nav
        aria-label={t("tabs.aria")}
        aria-orientation="vertical"
        className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-1.5 py-1"
        role="tablist"
      >
        <div className="flex flex-col gap-0.5">{primaryNav.map(renderNavItem)}</div>
        <SidebarSectionHeader id="network" label={t("tabs.network")}>
          <div className="flex flex-col gap-0.5">{networkNav.map(renderNavItem)}</div>
        </SidebarSectionHeader>
      </nav>

      <SidebarFooter onSelectPreset={onSelectPreset} />
    </aside>
  );
}
