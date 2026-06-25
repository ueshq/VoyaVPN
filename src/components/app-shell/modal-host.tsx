import { useEffect, useRef, useState, type FormEvent } from "react";
import { Info, KeyRound, Languages, Monitor, Moon, Settings, Sun, Type } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { fontOptions } from "@/config/fonts";
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
import {
  FONT_SIZE_MAX,
  FONT_SIZE_MIN,
  type Font,
  fontToClassName,
  type ThemeMode,
  usePreferencesStore,
} from "@/stores/preferences-store";
import { useModalStore } from "@/stores/modal-store";
import { useMountedRef } from "@/lib/use-mounted-ref";
import { getErrorMessage } from "@/lib/utils";

const themeOptions: Array<{ icon: typeof Monitor; labelKey: string; value: ThemeMode }> = [
  { icon: Monitor, labelKey: "menu.themeSystem", value: "system" },
  { icon: Sun, labelKey: "menu.themeLight", value: "light" },
  { icon: Moon, labelKey: "menu.themeDark", value: "dark" },
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
  const font = usePreferencesStore((state) => state.font);
  const fontSize = usePreferencesStore((state) => state.fontSize);
  const setFont = usePreferencesStore((state) => state.setFont);
  const setFontSize = usePreferencesStore((state) => state.setFontSize);
  const setThemeMode = usePreferencesStore((state) => state.setThemeMode);
  const themeMode = usePreferencesStore((state) => state.themeMode);
  const fontLabel = t("modal.font");
  const fontSizeLabel = t("modal.fontSize");

  return (
    <DialogContent className="max-h-[90dvh] w-[calc(100vw-2rem)] max-w-3xl gap-0 overflow-hidden p-0">
      <DialogHeader className="pe-12 px-6 pb-4 pt-6">
        <DialogTitle className="flex items-center gap-2">
          <Settings className="size-4" aria-hidden="true" />
          {t("modal.settings")}
        </DialogTitle>
        <DialogDescription className="sr-only">{t("modal.settingsDescription")}</DialogDescription>
      </DialogHeader>

      <div className="grid max-h-[calc(90dvh-5rem)] gap-5 overflow-y-auto px-6 pb-6">
        <section className="grid gap-3">
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
                  className="h-9 min-w-0 justify-start px-3"
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

        <section className="grid gap-3">
          <h3 className="flex items-center gap-2 text-sm font-medium">
            <Type className="size-4" aria-hidden="true" />
            {fontLabel}
          </h3>
          <div className="grid gap-3 sm:grid-cols-[1fr_10rem] sm:items-end">
            <div className="grid gap-2 sm:grid-cols-3">
              {fontOptions.map((option) => (
                <Button
                  key={option.value}
                  aria-pressed={font === option.value}
                  className="h-9 min-w-0 justify-start px-3"
                  onClick={() => setFont(option.value as Font)}
                  type="button"
                  variant={font === option.value ? "secondary" : "outline"}
                >
                  <span className={`${fontToClassName(option.value)} truncate`}>{option.label}</span>
                </Button>
              ))}
            </div>
            <div className="grid min-w-0 gap-1 text-sm">
              <Label className="text-muted-foreground">{fontSizeLabel}</Label>
              <Select onValueChange={(value) => setFontSize(Number(value))} value={String(fontSize)}>
                <SelectTrigger aria-label={fontSizeLabel} className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {fontSizeOptions.map((size) => (
                    <SelectItem key={size} value={String(size)}>
                      {size}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
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
                className="h-8 min-w-12 px-2 text-xs"
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
  const collectionGenerationRef = useRef(0);
  const mountedRef = useMountedRef();

  useEffect(() => {
    const generation = ++collectionGenerationRef.current;
    const isCurrent = () => mountedRef.current && generation === collectionGenerationRef.current;

    void sudoBeginCollection()
      .then((response) => {
        if (isCurrent()) {
          setCollection(response);
        }
      })
      .catch((error: unknown) => {
        if (isCurrent()) {
          setError(getErrorMessage(error));
        }
      });

    return () => {
      collectionGenerationRef.current += 1;
    };
  }, [mountedRef]);

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
      setError(getErrorMessage(error));
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
      setError(getErrorMessage(error));
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
        <div className="grid gap-2 text-sm">
          <Label htmlFor="sudo-password">{t("modal.sudoPassword")}</Label>
          <Input
            autoComplete="current-password"
            disabled={collection?.state === "ready" || submitting}
            id="sudo-password"
            onChange={(event) => setPassword(event.target.value)}
            type="password"
            value={password}
          />
        </div>

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
