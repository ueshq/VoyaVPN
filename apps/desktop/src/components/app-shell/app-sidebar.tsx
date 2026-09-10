import type * as React from "react";
import { ArrowDown, ArrowUp, Home, LoaderCircle, Network, Plug, Power, Route, Settings, Shield, WifiOff } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { BrandMark } from "@/assets/brand-mark";
import { SidebarNavItem } from "@/components/app-shell/sidebar-nav-item";
import { cn } from "@voya/ui/lib/utils";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n";
import { useRuntimeEventStore } from "@/ipc";
import type { CoreState } from "@/ipc/bindings";
import { formatBytesPerSecond } from "@voya/utils/formatting";
import { type ShellTab, useShellStore } from "@/stores/shell-store";

// `id` of the content `tabpanel` the nav controls. Exported so the shell can tag
// the panel element with a matching id for `aria-controls` / `aria-labelledby`.
export const SHELL_PANEL_ID = "shell-tabpanel";

type NavItem = { icon: LucideIcon; titleKey: TranslationKey; value: ShellTab };

// Flat six-destination nav (Hiddify-style): no grouping, no collapsing.
const navItems: NavItem[] = [
  { icon: Home, titleKey: "tabs.home", value: "home" },
  { icon: Network, titleKey: "tabs.proxies", value: "proxies" },
  { icon: Shield, titleKey: "tabs.profiles", value: "profiles" },
  { icon: Settings, titleKey: "tabs.settings", value: "settings" },
  { icon: Plug, titleKey: "tabs.connections", value: "connections" },
  { icon: Route, titleKey: "tabs.rules", value: "rules" },
];

const CORE_STATE_TRANSLATION_KEYS = {
  cleanupPending: "home.cleanupPending",
  connected: "status.connected",
  connecting: "status.connecting",
  disconnected: "status.disconnected",
  disconnecting: "status.disconnecting",
} as const satisfies Record<CoreState, TranslationKey>;

export function AppSidebar() {
  const { t } = useI18n();
  const activeTab = useShellStore((state) => state.activeTab);
  const requestTab = useShellStore((state) => state.requestTab);

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
    <aside className="flex h-full min-h-0 w-72 flex-col border-e border-sidebar-border bg-sidebar text-sidebar-foreground">
      {/* Brand block: a plain label, not a heading — the page-level h1 lives in
          the content area (Home renders the app name as its PageTitle). */}
      <div className="flex shrink-0 items-center gap-3 px-5 pt-5 pb-4">
        <BrandMark className="size-9 shrink-0 rounded-xl" aria-hidden="true" />
        <p className="truncate text-base font-semibold leading-none">{t("app.name")}</p>
      </div>

      <nav
        aria-label={t("tabs.aria")}
        aria-orientation="vertical"
        className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-3 py-2"
        onKeyDown={handleNavKeyDown}
        role="tablist"
      >
        {navItems.map((item) => (
          <SidebarNavItem
            key={item.value}
            active={activeTab === item.value}
            icon={item.icon}
            id={`shell-tab-${item.value}`}
            label={t(item.titleKey)}
            onSelect={() => requestTab(item.value)}
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
  const StateIcon = state === "connected" ? Power : state === "disconnected" ? WifiOff : LoaderCircle;
  const inTransition = state === "connecting" || state === "disconnecting";
  const uploadLabel = t("status.upload", { speed: formatBytesPerSecond(statistics?.uploadBytesPerSecond ?? 0) });
  const downloadLabel = t("status.download", { speed: formatBytesPerSecond(statistics?.downloadBytesPerSecond ?? 0) });

  return (
    <div
      aria-label={t("status.aria")}
      className="mt-auto shrink-0 border-t border-sidebar-border px-5 py-4 text-xs"
      data-testid="sidebar-footer"
    >
      <div
        className={cn(
          "flex items-center gap-2 font-medium",
          state === "connected" ? "text-connected" : "text-sidebar-foreground",
        )}
      >
        <StateIcon className={cn("size-3.5 shrink-0", inTransition && "animate-spin")} aria-hidden="true" />
        <span className="truncate">{t(CORE_STATE_TRANSLATION_KEYS[state])}</span>
      </div>
      <div className="mt-2 flex flex-col gap-1 text-muted-foreground">
        <span className="flex items-center gap-2">
          <ArrowUp className="size-3.5 shrink-0" aria-hidden="true" />
          <span className="min-w-0 truncate font-mono tabular-nums">{uploadLabel}</span>
        </span>
        <span className="flex items-center gap-2">
          <ArrowDown className="size-3.5 shrink-0" aria-hidden="true" />
          <span className="min-w-0 truncate font-mono tabular-nums">{downloadLabel}</span>
        </span>
      </div>
    </div>
  );
}
