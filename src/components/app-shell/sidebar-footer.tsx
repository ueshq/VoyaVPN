import { Database, Globe2, HelpCircle, Moon, QrCode, RefreshCw, Settings, Sun } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarTrigger,
} from "@/components/ui/menubar";
import { useI18n } from "@/i18n/use-i18n";
import type { PresetType } from "@/ipc/bindings";
import { useModalStore } from "@/stores/modal-store";
import { resolveThemeMode, type ThemeMode, usePreferencesStore } from "@/stores/preferences-store";

// The former Menubar Tools/Help actions now live as a row of icon actions pinned
// to the bottom of the sidebar: the static app actions (Backup, Check updates,
// QR, About), a one-tap theme toggle, and the Settings entry. Regional presets
// keep their multi-choice picker via a small Menubar dropdown (the same pattern
// the Columns menus already use) since picking a preset is a one-of-three choice.
export type RegionalPresetOption = {
  descriptionKey: string;
  labelKey: string;
  value: PresetType;
};

const regionalPresetOptions: RegionalPresetOption[] = [
  {
    descriptionKey: "modal.regionalPresetDefaultDescription",
    labelKey: "menu.regionalPresetDefault",
    value: 0,
  },
  {
    descriptionKey: "modal.regionalPresetRussiaDescription",
    labelKey: "menu.regionalPresetRussia",
    value: 1,
  },
  {
    descriptionKey: "modal.regionalPresetIranDescription",
    labelKey: "menu.regionalPresetIran",
    value: 2,
  },
];

export function SidebarFooter({
  onSelectPreset,
}: {
  onSelectPreset: (option: RegionalPresetOption) => void;
}) {
  const { t } = useI18n();
  const openModal = useModalStore((state) => state.openModal);
  const setThemeMode = usePreferencesStore((state) => state.setThemeMode);
  const themeMode = usePreferencesStore((state) => state.themeMode);
  const resolvedTheme = resolveThemeMode(themeMode);
  const nextThemeMode: ThemeMode = resolvedTheme === "dark" ? "light" : "dark";
  const ThemeIcon = resolvedTheme === "dark" ? Sun : Moon;

  return (
    <div className="flex shrink-0 flex-wrap items-center gap-1 border-t border-sidebar-border px-2 py-2">
      <Menubar className="h-8 border-0 bg-transparent p-0 shadow-none">
        <MenubarMenu>
          <MenubarTrigger
            aria-label={t("menu.regionalPresets")}
            className="size-8 justify-center rounded-md p-0 text-muted-foreground"
            title={t("menu.regionalPresets")}
          >
            <Globe2 className="size-4" aria-hidden="true" />
          </MenubarTrigger>
          <MenubarContent align="start">
            {regionalPresetOptions.map((option) => (
              <MenubarItem key={option.value} onSelect={() => onSelectPreset(option)}>
                {t(option.labelKey)}
              </MenubarItem>
            ))}
          </MenubarContent>
        </MenubarMenu>
      </Menubar>

      <SidebarFooterAction icon={Database} label={t("menu.backup")} onClick={() => openModal("backup")} />
      <SidebarFooterAction
        icon={RefreshCw}
        label={t("menu.checkUpdates")}
        onClick={() => openModal("updates")}
      />
      <SidebarFooterAction icon={QrCode} label={t("menu.qr")} onClick={() => openModal("qr")} />
      <SidebarFooterAction
        icon={ThemeIcon}
        label={t("menu.theme")}
        onClick={() => setThemeMode(nextThemeMode)}
      />
      <SidebarFooterAction
        icon={Settings}
        label={t("actions.settings")}
        onClick={() => openModal("settings")}
      />
      <SidebarFooterAction icon={HelpCircle} label={t("menu.about")} onClick={() => openModal("about")} />
    </div>
  );
}

function SidebarFooterAction({
  icon: Icon,
  label,
  onClick,
}: {
  icon: LucideIcon;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      aria-label={label}
      className="text-muted-foreground"
      onClick={onClick}
      size="icon-sm"
      title={label}
      type="button"
      variant="ghost"
    >
      <Icon className="size-4" aria-hidden="true" />
    </Button>
  );
}
