import { useEffect, useState, type FormEvent } from "react";
import { Info, KeyRound, Languages, Monitor, Moon, Palette, Settings, Sun, Type } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import { BackupDialog } from "@/features/backup";
import { IntegrationSettings, SourceSettings } from "@/features/options";
import { QrDialog } from "@/features/qr";
import { CheckUpdateDialog } from "@/features/updates";
import { useI18n } from "@/i18n/use-i18n";
import {
  setTunEnabled,
  sudoBeginCollection,
  sudoClearPassword,
  sudoSubmitPassword,
  useRuntimeEventStore,
} from "@/ipc";
import type { SudoCollectionResponse } from "@/ipc/bindings";
import { cn } from "@/lib/utils";
import {
  FONT_SIZE_MAX,
  FONT_SIZE_MIN,
  type Accent,
  type ThemeMode,
  usePreferencesStore,
} from "@/stores/preferences-store";
import { useModalStore } from "@/stores/modal-store";

const themeOptions: Array<{ icon: typeof Monitor; labelKey: string; value: ThemeMode }> = [
  { icon: Monitor, labelKey: "menu.themeSystem", value: "system" },
  { icon: Sun, labelKey: "menu.themeLight", value: "light" },
  { icon: Moon, labelKey: "menu.themeDark", value: "dark" },
];

const accentOptions: Array<{ className: string; labelKey: string; value: Accent }> = [
  { className: "bg-teal-600", labelKey: "menu.accentTeal", value: "teal" },
  { className: "bg-sky-600", labelKey: "menu.accentBlue", value: "blue" },
  { className: "bg-rose-600", labelKey: "menu.accentRose", value: "rose" },
];

const fontSizeOptions = Array.from(
  { length: FONT_SIZE_MAX - FONT_SIZE_MIN + 1 },
  (_, index) => FONT_SIZE_MIN + index,
);

export function ModalHost() {
  const closeTopModal = useModalStore((state) => state.closeTopModal);
  const stack = useModalStore((state) => state.stack);
  const modal = stack.at(-1);

  return (
    <Dialog open={Boolean(modal)} onOpenChange={(open) => !open && closeTopModal()}>
      {modal?.kind === "settings" ? <SettingsDialog /> : null}
      {modal?.kind === "about" ? <AboutDialog /> : null}
      {modal?.kind === "backup" ? <BackupDialog /> : null}
      {modal?.kind === "qr" ? <QrDialog /> : null}
      {modal?.kind === "sudo" ? <SudoPromptDialog intent={modal.intent} /> : null}
      {modal?.kind === "updates" ? <CheckUpdateDialog /> : null}
    </Dialog>
  );
}

function SettingsDialog() {
  const { language, localeOptions, setLocale, t } = useI18n();
  const accent = usePreferencesStore((state) => state.accent);
  const fontFamily = usePreferencesStore((state) => state.fontFamily);
  const fontSize = usePreferencesStore((state) => state.fontSize);
  const setAccent = usePreferencesStore((state) => state.setAccent);
  const setFontFamily = usePreferencesStore((state) => state.setFontFamily);
  const setFontSize = usePreferencesStore((state) => state.setFontSize);
  const setThemeMode = usePreferencesStore((state) => state.setThemeMode);
  const themeMode = usePreferencesStore((state) => state.themeMode);
  const fontFamilyLabel = stripParenthetical(t("resx.TbSettingsCurrentFontFamily"));
  const fontSizeLabel = t("resx.TbSettingsFontSize");

  return (
    <DialogContent className="max-h-[90vh] w-[min(94vw,44rem)] overflow-y-auto">
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <Settings className="size-4" aria-hidden="true" />
          {t("modal.settings")}
        </DialogTitle>
        <DialogDescription className="sr-only">{t("modal.settingsDescription")}</DialogDescription>
      </DialogHeader>

      <div className="grid gap-5">
        <section className="grid gap-2">
          <h3 className="flex items-center gap-2 text-sm font-medium">
            <Monitor className="size-4" aria-hidden="true" />
            {t("modal.theme")}
          </h3>
          <div className="grid gap-2 sm:grid-cols-3">
            {themeOptions.map((option) => {
              const Icon = option.icon;

              return (
                <Button
                  key={option.value}
                  aria-pressed={themeMode === option.value}
                  className="min-w-0 justify-start"
                  onClick={() => setThemeMode(option.value)}
                  type="button"
                  variant={themeMode === option.value ? "secondary" : "outline"}
                >
                  <Icon className="size-4" aria-hidden="true" />
                  <span className="truncate">{t(option.labelKey)}</span>
                </Button>
              );
            })}
          </div>
        </section>

        <Separator />

        <section className="grid gap-2">
          <h3 className="flex items-center gap-2 text-sm font-medium">
            <Palette className="size-4" aria-hidden="true" />
            {t("modal.accent")}
          </h3>
          <div className="grid gap-2 sm:grid-cols-3">
            {accentOptions.map((option) => (
              <Button
                key={option.value}
                aria-pressed={accent === option.value}
                className="min-w-0 justify-start"
                onClick={() => setAccent(option.value)}
                type="button"
                variant={accent === option.value ? "secondary" : "outline"}
              >
                <span className={cn("size-3 rounded-full", option.className)} aria-hidden="true" />
                <span className="truncate">{t(option.labelKey)}</span>
              </Button>
            ))}
          </div>
        </section>

        <Separator />

        <section className="grid gap-3">
          <h3 className="flex items-center gap-2 text-sm font-medium">
            <Type className="size-4" aria-hidden="true" />
            {fontSizeLabel}
          </h3>
          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_8rem]">
            <label className="grid min-w-0 gap-1 text-sm">
              <span className="text-muted-foreground">{fontFamilyLabel}</span>
              <input
                aria-label={fontFamilyLabel}
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                onChange={(event) => setFontFamily(event.currentTarget.value)}
                value={fontFamily}
              />
            </label>
            <label className="grid min-w-0 gap-1 text-sm">
              <span className="text-muted-foreground">{fontSizeLabel}</span>
              <select
                aria-label={fontSizeLabel}
                className="h-9 min-w-0 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                onChange={(event) => setFontSize(Number(event.currentTarget.value))}
                value={fontSize}
              >
                {fontSizeOptions.map((size) => (
                  <option key={size} value={size}>
                    {size}
                  </option>
                ))}
              </select>
            </label>
          </div>
        </section>

        <Separator />

        <SourceSettings />

        <Separator />

        <IntegrationSettings />

        <Separator />

        <section className="grid gap-2">
          <h3 className="flex items-center gap-2 text-sm font-medium">
            <Languages className="size-4" aria-hidden="true" />
            {t("modal.language")}
          </h3>
          <div className="flex flex-wrap gap-2">
            {localeOptions.map((locale) => (
              <Button
                key={locale.code}
                aria-pressed={language === locale.code}
                onClick={() => void setLocale(locale.code)}
                type="button"
                variant={language === locale.code ? "secondary" : "outline"}
              >
                {locale.label}
              </Button>
            ))}
          </div>
        </section>
      </div>
    </DialogContent>
  );
}

function stripParenthetical(value: string) {
  return value.replace(/\s*[(（][^)）]*[)）]\s*$/u, "");
}

function AboutDialog() {
  const { t } = useI18n();

  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <Info className="size-4" aria-hidden="true" />
          {t("modal.about")}
        </DialogTitle>
        <DialogDescription className="sr-only">{t("modal.aboutDescription")}</DialogDescription>
      </DialogHeader>
      <div className="grid gap-2 text-sm">
        <p className="font-medium">{t("app.name")}</p>
        <p className="text-muted-foreground">{t("modal.version")}</p>
      </div>
      <DialogFooter />
    </DialogContent>
  );
}

function SudoPromptDialog({ intent }: { intent?: "enableTun" }) {
  const { t } = useI18n();
  const closeTopModal = useModalStore((state) => state.closeTopModal);
  const setTun = useRuntimeEventStore((state) => state.setTun);
  const [collection, setCollection] = useState<SudoCollectionResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    let disposed = false;

    void sudoBeginCollection()
      .then((response) => {
        if (!disposed) {
          setCollection(response);
        }
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  async function submitPassword(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!collection?.requestId) {
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      const response = await sudoSubmitPassword(collection.requestId, password);
      setCollection(response);
      setPassword("");
      if (response.state === "ready") {
        if (intent === "enableTun") {
          const status = await setTunEnabled(true);
          setTun({ enabled: status.enabled });
        }
        closeTopModal();
      }
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  }

  async function clearPassword() {
    setSubmitting(true);
    setError(null);
    try {
      await sudoClearPassword();
      setCollection(await sudoBeginCollection());
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <KeyRound className="size-4" aria-hidden="true" />
          {t("modal.sudo")}
        </DialogTitle>
        <DialogDescription>{t("modal.sudoDescription")}</DialogDescription>
      </DialogHeader>

      <form className="grid gap-4" onSubmit={(event) => void submitPassword(event)}>
        <label className="grid gap-2 text-sm">
          <span className="font-medium">{t("modal.sudoPassword")}</span>
          <input
            autoComplete="current-password"
            className="h-9 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
            disabled={collection?.state === "ready" || submitting}
            onChange={(event) => setPassword(event.target.value)}
            type="password"
            value={password}
          />
        </label>

        {collection?.state === "ready" ? (
          <p className="text-sm text-muted-foreground">{t("modal.sudoReady")}</p>
        ) : null}
        {error ? <p className="text-sm text-destructive">{error}</p> : null}

        <DialogFooter>
          <Button disabled={submitting} onClick={() => void clearPassword()} type="button" variant="outline">
            {t("modal.sudoClear")}
          </Button>
          <Button disabled={!collection?.requestId || !password || submitting} type="submit">
            {t("modal.sudoSubmit")}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  );
}
