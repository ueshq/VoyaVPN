import { useEffect, useMemo, useRef, useState, type MutableRefObject } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  Database,
  FilePlus2,
  FolderInput,
  Globe2,
  HelpCircle,
  Home,
  Languages,
  Monitor,
  Moon,
  Network,
  Plug,
  QrCode,
  RefreshCw,
  Route,
  ScrollText,
  Settings,
  Shield,
  Sun,
  Type,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { ModalHost } from "@/components/app-shell/modal-host";
import { StatusBar } from "@/components/app-shell/status-bar";
import { Toaster } from "@/components/app-shell/toaster";
import { fontOptions, fonts } from "@/config/fonts";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Menubar,
  MenubarContent,
  MenubarItem,
  MenubarMenu,
  MenubarRadioGroup,
  MenubarRadioItem,
  MenubarSeparator,
  MenubarSub,
  MenubarSubContent,
  MenubarSubTrigger,
  MenubarTrigger,
} from "@/components/ui/menubar";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { applyDocumentLocale } from "@/i18n";
import { useI18n } from "@/i18n/use-i18n";
import { HomeScreen } from "@/features/home";
import { ProfilesScreen } from "@/features/profiles";
import { RoutingScreen } from "@/features/routing";
import { DnsScreen } from "@/features/dns";
import { ClashConnectionsScreen, ClashProxiesScreen } from "@/features/clash";
import { LogsScreen } from "@/features/logs";
import {
  applyRegionalPreset,
  clashStartMonitor,
  clashStopMonitor,
  loadAppConfig,
  saveAppConfig,
  useRuntimeEventStore,
} from "@/ipc";
import type { AppConfig_Deserialize, ClashMonitorStatus, PresetType } from "@/ipc/bindings";
import { useMountedRef } from "@/lib/use-mounted-ref";
import { getErrorMessage } from "@/lib/utils";
import {
  type Font,
  fontToClassName,
  fontToCss,
  fontToFamilyString,
  resolveThemeMode,
  themeModeToConfig,
  type ThemeMode,
  uiItemWithoutLegacyColor,
  usePreferencesStore,
} from "@/stores/preferences-store";
import { type ShellTab, useShellStore } from "@/stores/shell-store";
import { type ModalKind, useModalStore } from "@/stores/modal-store";
import { useToastStore } from "@/stores/toast-store";

type ShellTabConfig = {
  emptyKey: string;
  icon: LucideIcon;
  primaryActionKey?: string;
  secondaryActionKey?: string;
  titleKey: string;
  value: ShellTab;
};

const shellTabs: ShellTabConfig[] = [
  {
    emptyKey: "home.aria",
    icon: Home,
    titleKey: "tabs.home",
    value: "home",
  },
  {
    emptyKey: "panes.profiles.empty",
    icon: Shield,
    primaryActionKey: "actions.addProfile",
    secondaryActionKey: "actions.import",
    titleKey: "tabs.profiles",
    value: "profiles",
  },
  {
    emptyKey: "panes.routing.empty",
    icon: Route,
    primaryActionKey: "actions.refresh",
    titleKey: "tabs.routing",
    value: "routing",
  },
  {
    emptyKey: "panes.dns.empty",
    icon: Database,
    primaryActionKey: "actions.refresh",
    titleKey: "tabs.dns",
    value: "dns",
  },
  {
    emptyKey: "panes.clashProxies.empty",
    icon: Network,
    primaryActionKey: "actions.refresh",
    titleKey: "tabs.clashProxies",
    value: "clash-proxies",
  },
  {
    emptyKey: "panes.clashConnections.empty",
    icon: Plug,
    primaryActionKey: "actions.refresh",
    secondaryActionKey: "actions.clear",
    titleKey: "tabs.clashConnections",
    value: "clash-connections",
  },
  {
    emptyKey: "panes.logs.empty",
    icon: ScrollText,
    primaryActionKey: "actions.pause",
    secondaryActionKey: "actions.clear",
    titleKey: "tabs.logs",
    value: "logs",
  },
];

const themeMenuOptions: Array<{ icon: LucideIcon; labelKey: string; value: ThemeMode }> = [
  { icon: Monitor, labelKey: "menu.themeSystem", value: "system" },
  { icon: Sun, labelKey: "menu.themeLight", value: "light" },
  { icon: Moon, labelKey: "menu.themeDark", value: "dark" },
];

type RegionalPresetOption = {
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

export function AppShell() {
  const queryClient = useQueryClient();
  const { direction, language, localeOptions, setLocale, t } = useI18n();
  const font = usePreferencesStore((state) => state.font);
  const fontSize = usePreferencesStore((state) => state.fontSize);
  const activeTab = useShellStore((state) => state.activeTab);
  const openModal = useModalStore((state) => state.openModal);
  const pushToast = useToastStore((state) => state.pushToast);
  const setActiveTab = useShellStore((state) => state.setActiveTab);
  const setFont = usePreferencesStore((state) => state.setFont);
  const setThemeMode = usePreferencesStore((state) => state.setThemeMode);
  const themeMode = usePreferencesStore((state) => state.themeMode);
  const [activeMenu, setActiveMenu] = useState("");
  const [pendingPreset, setPendingPreset] = useState<RegionalPresetOption | null>(null);
  const fontLabel = t("menu.font");

  usePersistedPreferences(language);
  useThemeEffects(themeMode, font, fontSize);
  useClashMonitorLifecycle(activeTab);

  useEffect(() => {
    applyDocumentLocale(language);
  }, [language]);

  async function handleRegionalPresetApplied(fallbackCustomDnsEnabled: boolean) {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["app-config"] }),
      queryClient.invalidateQueries({ queryKey: ["dns"] }),
      queryClient.invalidateQueries({ queryKey: ["routings"] }),
    ]);
    pushToast({
      description: fallbackCustomDnsEnabled
        ? t("modal.regionalPresetAppliedFallback")
        : t("modal.regionalPresetAppliedDescription"),
      title: t("modal.regionalPresetApplied"),
    });
  }

  function openShellModal(kind: ModalKind) {
    setActiveMenu("");
    openModal(kind);
  }

  function selectMenuTab(tab: ShellTab) {
    setActiveMenu("");
    setActiveTab(tab);
  }

  function selectRegionalPreset(option: RegionalPresetOption) {
    setActiveMenu("");
    setPendingPreset(option);
  }

  return (
    <main className="min-h-screen bg-background text-foreground" dir={direction}>
      <div className="flex h-screen min-h-[34rem] flex-col overflow-hidden">
        <header className="shrink-0 border-b border-border bg-card text-card-foreground">
          <div className="flex h-14 items-center gap-2 px-4">
            <div className="flex min-w-0 items-center gap-3">
              <div className="flex size-8 shrink-0 items-center justify-center rounded-md border border-border bg-background text-muted-foreground">
                <Shield className="size-4" aria-hidden="true" />
              </div>
              <h1 className="truncate text-sm font-semibold leading-none">{t("app.name")}</h1>
            </div>

            <Menubar
              className="ms-2 hidden h-8 shrink-0 p-0.5 lg:flex"
              onValueChange={setActiveMenu}
              value={activeMenu}
            >
              <MenubarMenu>
                <MenubarTrigger>{t("menu.file")}</MenubarTrigger>
                <MenubarContent>
                  <MenubarItem disabled>
                    <FilePlus2 className="size-4" aria-hidden="true" />
                    {t("menu.newProfile")}
                  </MenubarItem>
                  <MenubarItem disabled>
                    <FolderInput className="size-4" aria-hidden="true" />
                    {t("menu.import")}
                  </MenubarItem>
                  <MenubarSeparator />
                  <MenubarItem disabled>{t("menu.quit")}</MenubarItem>
                </MenubarContent>
              </MenubarMenu>

              <MenubarMenu>
                <MenubarTrigger>{t("menu.view")}</MenubarTrigger>
                <MenubarContent>
                  <MenubarSub>
                    <MenubarSubTrigger>
                      <Languages className="size-4" aria-hidden="true" />
                      {t("menu.language")}
                    </MenubarSubTrigger>
                    <MenubarSubContent>
                      <MenubarRadioGroup
                        onValueChange={(value) => void setLocale(value as typeof language)}
                        value={language}
                      >
                        {localeOptions.map((locale) => (
                          <MenubarRadioItem key={locale.code} value={locale.code}>
                            {locale.label}
                          </MenubarRadioItem>
                        ))}
                      </MenubarRadioGroup>
                    </MenubarSubContent>
                  </MenubarSub>
                  <MenubarSub>
                    <MenubarSubTrigger>
                      <Monitor className="size-4" aria-hidden="true" />
                      {t("menu.theme")}
                    </MenubarSubTrigger>
                    <MenubarSubContent>
                      <MenubarRadioGroup
                        onValueChange={(value) => setThemeMode(value as ThemeMode)}
                        value={themeMode}
                      >
                        {themeMenuOptions.map((option) => {
                          const Icon = option.icon;

                          return (
                            <MenubarRadioItem key={option.value} value={option.value}>
                              <Icon className="size-4" aria-hidden="true" />
                              {t(option.labelKey)}
                            </MenubarRadioItem>
                          );
                        })}
                      </MenubarRadioGroup>
                    </MenubarSubContent>
                  </MenubarSub>
                  <MenubarSub>
                    <MenubarSubTrigger>
                      <Type className="size-4" aria-hidden="true" />
                      {fontLabel}
                    </MenubarSubTrigger>
                    <MenubarSubContent>
                      <MenubarRadioGroup onValueChange={(value) => setFont(value as Font)} value={font}>
                        {fontOptions.map((option) => (
                          <MenubarRadioItem key={option.value} value={option.value}>
                            <span className={fontToClassName(option.value)}>{option.label}</span>
                          </MenubarRadioItem>
                        ))}
                      </MenubarRadioGroup>
                    </MenubarSubContent>
                  </MenubarSub>
                </MenubarContent>
              </MenubarMenu>

              <MenubarMenu>
                <MenubarTrigger>{t("menu.tools")}</MenubarTrigger>
                <MenubarContent>
                  <MenubarItem onSelect={() => selectMenuTab("routing")}>
                    <Route className="size-4" aria-hidden="true" />
                    {t("menu.routing")}
                  </MenubarItem>
                  <MenubarItem onSelect={() => selectMenuTab("dns")}>
                    <Database className="size-4" aria-hidden="true" />
                    {t("menu.dns")}
                  </MenubarItem>
                  <MenubarSub>
                    <MenubarSubTrigger>
                      <Globe2 className="size-4" aria-hidden="true" />
                      {t("menu.regionalPresets")}
                    </MenubarSubTrigger>
                    <MenubarSubContent>
                      {regionalPresetOptions.map((option) => (
                        <MenubarItem key={option.value} onSelect={() => selectRegionalPreset(option)}>
                          {t(option.labelKey)}
                        </MenubarItem>
                      ))}
                    </MenubarSubContent>
                  </MenubarSub>
                  <MenubarItem disabled>{t("menu.systemProxy")}</MenubarItem>
                  <MenubarItem disabled>{t("menu.tun")}</MenubarItem>
                  <MenubarItem disabled>{t("menu.clash")}</MenubarItem>
                  <MenubarItem onSelect={() => openShellModal("backup")}>
                    <Database className="size-4" aria-hidden="true" />
                    {t("menu.backup")}
                  </MenubarItem>
                  <MenubarItem onSelect={() => openShellModal("updates")}>
                    <RefreshCw className="size-4" aria-hidden="true" />
                    {t("menu.checkUpdates")}
                  </MenubarItem>
                  <MenubarItem onSelect={() => openShellModal("qr")}>
                    <QrCode className="size-4" aria-hidden="true" />
                    {t("menu.qr")}
                  </MenubarItem>
                </MenubarContent>
              </MenubarMenu>

              <MenubarMenu>
                <MenubarTrigger>{t("menu.help")}</MenubarTrigger>
                <MenubarContent>
                  <MenubarItem onSelect={() => openShellModal("about")}>
                    <HelpCircle className="size-4" aria-hidden="true" />
                    {t("menu.about")}
                  </MenubarItem>
                </MenubarContent>
              </MenubarMenu>
            </Menubar>

            <div className="ms-auto flex min-w-0 items-center gap-2">
              <div
                className="hidden shrink-0 rounded-md border border-border bg-background p-0.5 md:flex"
                aria-label={t("menu.language")}
              >
                {localeOptions.map((locale) => (
                  <Button
                    key={locale.code}
                    aria-pressed={language === locale.code}
                    className="h-7 min-w-7 px-2 text-xs"
                    onClick={() => void setLocale(locale.code)}
                    title={locale.code}
                    type="button"
                    variant={language === locale.code ? "secondary" : "ghost"}
                  >
                    {locale.label}
                  </Button>
                ))}
              </div>
              <Button
                aria-label={t("actions.settings")}
                className="h-8 shrink-0 gap-2 px-3"
                onClick={() => openModal("settings")}
                size="sm"
                variant="outline"
              >
                <Settings className="size-4" aria-hidden="true" />
                <span className="hidden max-w-28 truncate sm:inline">{t("actions.settings")}</span>
              </Button>
            </div>
          </div>
        </header>

        <Tabs
          className="flex min-h-0 flex-1 flex-col gap-0"
          onValueChange={(value) => setActiveTab(value as ShellTab)}
          value={activeTab}
        >
          <div className="shrink-0 overflow-x-auto border-b border-border bg-card px-4 py-1.5">
            <TabsList aria-label={t("tabs.aria")} className="min-w-max">
              {shellTabs.map((tab) => {
                const Icon = tab.icon;

                return (
                  <TabsTrigger key={tab.value} value={tab.value}>
                    <Icon className="size-4" aria-hidden="true" />
                    <span>{t(tab.titleKey)}</span>
                  </TabsTrigger>
                );
              })}
            </TabsList>
          </div>

          <div className="min-h-0 flex-1 bg-background">
            {shellTabs.map((tab) => (
              <TabsContent key={tab.value} className="m-0 h-full" value={tab.value}>
                {tab.value === "home" ? (
                  <HomeScreen />
                ) : tab.value === "profiles" ? (
                  <ProfilesScreen />
                ) : tab.value === "routing" ? (
                  <RoutingScreen />
                ) : tab.value === "dns" ? (
                  <DnsScreen />
                ) : tab.value === "clash-proxies" ? (
                  <ClashProxiesScreen />
                ) : tab.value === "clash-connections" ? (
                  <ClashConnectionsScreen />
                ) : tab.value === "logs" ? (
                  <LogsScreen />
                ) : (
                  <ShellPane tab={tab} />
                )}
              </TabsContent>
            ))}
          </div>
        </Tabs>

        <StatusBar />
      </div>

      <ModalHost />
      <RegionalPresetConfirmDialog
        onApplied={(fallbackCustomDnsEnabled) => void handleRegionalPresetApplied(fallbackCustomDnsEnabled)}
        onOpenChange={(open) => {
          if (!open) {
            setPendingPreset(null);
          }
        }}
        preset={pendingPreset}
      />
      <Toaster />
    </main>
  );
}

function RegionalPresetConfirmDialog({
  onApplied,
  onOpenChange,
  preset,
}: {
  onApplied: (fallbackCustomDnsEnabled: boolean) => void;
  onOpenChange: (open: boolean) => void;
  preset: RegionalPresetOption | null;
}) {
  const { t } = useI18n();
  const [error, setError] = useState<string | null>(null);
  const [isApplying, setIsApplying] = useState(false);
  const open = Boolean(preset);
  const title = useMemo(
    () => (preset ? t("modal.regionalPresetTitle", { preset: t(preset.labelKey) }) : ""),
    [preset, t],
  );

  async function confirmApply() {
    if (!preset) {
      return;
    }

    setIsApplying(true);
    setError(null);
    try {
      const result = await applyRegionalPreset(preset.value, true, null);
      onApplied(result.fallbackCustomDnsEnabled);
      onOpenChange(false);
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setIsApplying(false);
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!isApplying) {
          setError(null);
          onOpenChange(nextOpen);
        }
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Globe2 className="size-4" aria-hidden="true" />
            {title}
          </DialogTitle>
          <DialogDescription>
            {preset ? t(preset.descriptionKey) : t("modal.regionalPresetDescription")}
          </DialogDescription>
        </DialogHeader>

        {error ? (
          <div className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            {error}
          </div>
        ) : null}

        <DialogFooter>
          <Button disabled={isApplying} onClick={() => onOpenChange(false)} type="button" variant="outline">
            {t("actions.close")}
          </Button>
          <Button disabled={!preset || isApplying} onClick={() => void confirmApply()} type="button">
            {isApplying ? t("modal.regionalPresetApplying") : t("modal.regionalPresetApply")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function ShellPane({ tab }: { tab: ShellTabConfig }) {
  const { t } = useI18n();
  const Icon = tab.icon;

  return (
    <section className="flex h-full min-h-0 flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-border bg-card px-4">
        <h2 className="text-sm font-semibold">{t(tab.titleKey)}</h2>
        <div className="ms-auto flex items-center gap-2">
          {tab.secondaryActionKey ? (
            <Button disabled size="sm" type="button" variant="outline">
              {t(tab.secondaryActionKey)}
            </Button>
          ) : null}
          {tab.primaryActionKey ? (
            <Button disabled size="sm" type="button" variant="secondary">
              {t(tab.primaryActionKey)}
            </Button>
          ) : null}
        </div>
      </div>

      <div className="grid min-h-0 flex-1 place-items-center p-6">
        <div className="grid justify-items-center gap-3 text-center">
          <div className="flex size-10 items-center justify-center rounded-md border border-border bg-card">
            <Icon className="size-5 text-muted-foreground" aria-hidden="true" />
          </div>
          <p className="text-sm font-medium">{t(tab.emptyKey)}</p>
        </div>
      </div>
    </section>
  );
}

function usePersistedPreferences(language: string) {
  const appConfigLoaded = usePreferencesStore((state) => state.appConfigLoaded);
  const font = usePreferencesStore((state) => state.font);
  const fontSize = usePreferencesStore((state) => state.fontSize);
  const hydrateFromConfig = usePreferencesStore((state) => state.hydrateFromConfig);
  const themeMode = usePreferencesStore((state) => state.themeMode);
  const lastPersistedKeyRef = useRef<string | null>(null);
  const loadGenerationRef = useRef(0);
  const mountedRef = useMountedRef();
  const persistQueueRef = useRef<Promise<void>>(Promise.resolve());
  const persistSequenceRef = useRef(0);
  const preferenceSnapshot = useMemo<PreferenceConfigSnapshot>(
    () => ({ font, fontSize, language, themeMode }),
    [font, fontSize, language, themeMode],
  );

  useEffect(() => {
    if (appConfigLoaded) {
      return undefined;
    }

    const generation = ++loadGenerationRef.current;

    void loadAppConfig()
      .then((config) => {
        if (!mountedRef.current || generation !== loadGenerationRef.current) {
          return;
        }

        hydrateFromConfig(config.UIItem);
        lastPersistedKeyRef.current = preferenceConfigKey({
          font: usePreferencesStore.getState().font,
          fontSize: usePreferencesStore.getState().fontSize,
          language,
          themeMode: usePreferencesStore.getState().themeMode,
        });
      })
      .catch(() => undefined);

    return () => {
      loadGenerationRef.current += 1;
    };
  }, [appConfigLoaded, hydrateFromConfig, language, mountedRef]);

  useEffect(() => {
    if (!appConfigLoaded || lastPersistedKeyRef.current === null) {
      return undefined;
    }

    const persistKey = preferenceConfigKey(preferenceSnapshot);
    if (lastPersistedKeyRef.current === persistKey) {
      return undefined;
    }

    const sequence = ++persistSequenceRef.current;
    const timeout = window.setTimeout(() => {
      persistQueueRef.current = persistQueueRef.current
        .catch(() => undefined)
        .then(() => persistPreferenceConfig(preferenceSnapshot))
        .then(() => {
          if (persistSequenceRef.current === sequence) {
            lastPersistedKeyRef.current = persistKey;
          }
        })
        .catch(() => undefined);
    }, 250);

    return () => window.clearTimeout(timeout);
  }, [appConfigLoaded, preferenceSnapshot]);
}

function useClashMonitorLifecycle(activeTab: ShellTab) {
  const pushToast = useToastStore((state) => state.pushToast);
  const startTimerRef = useRef<number | null>(null);
  const stopTimerRef = useRef<number | null>(null);
  const runningRef = useRef(false);
  const startingRef = useRef(false);
  const stoppingRef = useRef(false);
  const wantsMonitorRef = useRef(false);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return undefined;
    }

    wantsMonitorRef.current = isClashTab(activeTab);
    clearTimer(startTimerRef);
    clearTimer(stopTimerRef);

    if (wantsMonitorRef.current) {
      if (!runningRef.current && !startingRef.current && !stoppingRef.current) {
        scheduleClashMonitorStart({
          pushToast,
          runningRef,
          startingRef,
          startTimerRef,
          stoppingRef,
          stopTimerRef,
          wantsMonitorRef,
        });
      }

      return undefined;
    }

    if (runningRef.current || startingRef.current || stoppingRef.current) {
      scheduleClashMonitorStop({
        pushToast,
        runningRef,
        startingRef,
        startTimerRef,
        stoppingRef,
        stopTimerRef,
        wantsMonitorRef,
      });
    }

    return undefined;
  }, [activeTab, pushToast]);

  useEffect(
    () => () => {
      clearTimer(startTimerRef);
      clearTimer(stopTimerRef);
      wantsMonitorRef.current = false;
      if (runningRef.current) {
        void clashStopMonitor().catch(() => undefined);
      }
    },
    [],
  );
}

type PushToast = ReturnType<typeof useToastStore.getState>["pushToast"];

type ClashMonitorLifecycleRefs = {
  runningRef: MutableRefObject<boolean>;
  startingRef: MutableRefObject<boolean>;
  startTimerRef: MutableRefObject<number | null>;
  stoppingRef: MutableRefObject<boolean>;
  stopTimerRef: MutableRefObject<number | null>;
  wantsMonitorRef: MutableRefObject<boolean>;
};

function scheduleClashMonitorStart({
  pushToast,
  runningRef,
  startingRef,
  startTimerRef,
  stoppingRef,
  stopTimerRef,
  wantsMonitorRef,
}: ClashMonitorLifecycleRefs & { pushToast: PushToast }) {
  clearTimer(startTimerRef);
  startTimerRef.current = window.setTimeout(() => {
    startTimerRef.current = null;
    if (!wantsMonitorRef.current || runningRef.current || startingRef.current || stoppingRef.current) {
      return;
    }

    startingRef.current = true;
    useRuntimeEventStore.getState().setClashMonitorStarting();
    void clashStartMonitor()
      .then((status) => {
        applyClashMonitorStatus(status, runningRef);
        if (!wantsMonitorRef.current && status.running) {
          scheduleClashMonitorStop({
            pushToast,
            runningRef,
            startingRef,
            startTimerRef,
            stoppingRef,
            stopTimerRef,
            wantsMonitorRef,
          });
        }
      })
      .catch((error) => {
        const message = clashMonitorErrorMessage(error, "Unable to start Clash monitor.");

        runningRef.current = false;
        useRuntimeEventStore.getState().setClashMonitorFailed(message);
        pushToast({ description: message, title: "Clash" });
      })
      .finally(() => {
        startingRef.current = false;
      });
  }, 100);
}

function scheduleClashMonitorStop({
  pushToast,
  runningRef,
  startingRef,
  startTimerRef,
  stoppingRef,
  stopTimerRef,
  wantsMonitorRef,
}: ClashMonitorLifecycleRefs & { pushToast: PushToast }) {
  clearTimer(stopTimerRef);
  stopTimerRef.current = window.setTimeout(() => {
    stopTimerRef.current = null;
    if (!runningRef.current && !startingRef.current && !stoppingRef.current) {
      return;
    }

    stoppingRef.current = true;
    void clashStopMonitor()
      .then((status) => {
        applyClashMonitorStatus(status, runningRef);
      })
      .catch((error) => {
        const message = clashMonitorErrorMessage(error, "Unable to stop Clash monitor.");

        runningRef.current = false;
        useRuntimeEventStore.getState().setClashMonitorFailed(message);
        pushToast({ description: message, title: "Clash" });
      })
      .finally(() => {
        stoppingRef.current = false;
        if (wantsMonitorRef.current && !runningRef.current && !startingRef.current) {
          scheduleClashMonitorStart({
            pushToast,
            runningRef,
            startingRef,
            startTimerRef,
            stoppingRef,
            stopTimerRef,
            wantsMonitorRef,
          });
        }
      });
  }, 2_000);
}

function applyClashMonitorStatus(status: ClashMonitorStatus, runningRef: MutableRefObject<boolean>) {
  runningRef.current = status.running;
  useRuntimeEventStore.getState().setClashMonitorStatus(status);
}

function clashMonitorErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message) {
    return error.message;
  }

  if (typeof error === "string" && error) {
    return error;
  }

  return fallback;
}

function clearTimer(timerRef: MutableRefObject<number | null>) {
  if (timerRef.current !== null) {
    window.clearTimeout(timerRef.current);
    timerRef.current = null;
  }
}

function isClashTab(tab: ShellTab) {
  return tab === "clash-proxies" || tab === "clash-connections";
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

type PreferenceConfigSnapshot = {
  font: Font;
  fontSize: number;
  language: string;
  themeMode: ThemeMode;
};

function preferenceConfigKey({
  font,
  fontSize,
  language,
  themeMode,
}: PreferenceConfigSnapshot) {
  return JSON.stringify({
    font,
    fontSize,
    language,
    themeMode,
  });
}

async function persistPreferenceConfig({
  font,
  fontSize,
  language,
  themeMode,
}: PreferenceConfigSnapshot) {
  const config = await loadAppConfig();
  const nextConfig = {
    ...config,
    UIItem: {
      ...uiItemWithoutLegacyColor(config.UIItem),
      CurrentFontFamily: fontToFamilyString(font),
      CurrentFontSize: fontSize,
      CurrentLanguage: language,
      CurrentTheme: themeModeToConfig(themeMode),
    },
  } satisfies AppConfig_Deserialize;

  await saveAppConfig(nextConfig);
}

function useThemeEffects(themeMode: ThemeMode, font: Font, fontSize: number) {
  useEffect(() => {
    const root = document.documentElement;
    const media =
      typeof window.matchMedia === "function" ? window.matchMedia("(prefers-color-scheme: dark)") : undefined;

    const applyTheme = () => {
      const resolvedTheme = resolveThemeMode(themeMode);

      root.classList.toggle("dark", resolvedTheme === "dark");
      root.classList.remove(...fonts.map(fontToClassName));
      root.classList.add(fontToClassName(font));
      root.style.colorScheme = resolvedTheme;
      root.style.setProperty("--app-font-family", fontToCss(font));
      root.style.setProperty("--app-font-size", `${fontSize}px`);
    };

    applyTheme();

    if (themeMode !== "system" || !media) {
      return undefined;
    }

    media.addEventListener("change", applyTheme);

    return () => media.removeEventListener("change", applyTheme);
  }, [font, fontSize, themeMode]);
}
