import { useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { Activity, Keyboard, Power, Save } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { useI18n } from "@/i18n/use-i18n";
import {
  autostartStatus,
  diagnosticsStatus,
  globalHotkeyStatus,
  saveGlobalHotkeys,
  setDiagnosticsEnabled,
  setAutostartEnabled,
} from "@/ipc";
import type {
  AutostartStatus,
  DiagnosticsStatus,
  GlobalHotkey,
  HotkeyStatus_Serialize,
  KeyEventItem_Deserialize,
  KeyEventItem_Serialize,
} from "@/ipc/bindings";
import { redactOperationalError } from "@/lib/operational-redaction";
import { getErrorMessage } from "@/lib/utils";

type MutableKeyEventItem = Required<Pick<KeyEventItem_Deserialize, "Alt" | "Control" | "Shift">> & {
  EGlobalHotkey: GlobalHotkey;
  KeyCode: number | null;
};

export function IntegrationSettings() {
  const { t } = useI18n();
  const [autostart, setAutostart] = useState<AutostartStatus | null>(null);
  const [diagnostics, setDiagnostics] = useState<DiagnosticsStatus | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [diagnosticsWorking, setDiagnosticsWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hotkeys, setHotkeys] = useState<HotkeyStatus_Serialize | null>(null);
  const [settings, setSettings] = useState<MutableKeyEventItem[]>([]);
  const [saved, setSaved] = useState(false);
  const [working, setWorking] = useState(false);

  useEffect(() => {
    let disposed = false;

    void Promise.all([autostartStatus(), globalHotkeyStatus()])
      .then(([autostartStatusResult, hotkeyStatusResult]) => {
        if (disposed) {
          return;
        }
        setAutostart(autostartStatusResult);
        setHotkeys(hotkeyStatusResult);
        setSettings(hotkeyStatusResult.settings.map(toMutableSetting));
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setError(getErrorMessage(error));
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    let disposed = false;

    void diagnosticsStatus()
      .then((status) => {
        if (disposed) {
          return;
        }
        setDiagnostics(status);
        setDiagnosticsError(null);
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setDiagnosticsError(redactOperationalError(error));
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  const artifact = useMemo(() => {
    if (!autostart?.artifactPath) {
      return autostart?.artifactName ?? "";
    }

    return autostart.artifactName
      ? `${autostart.artifactName} · ${autostart.artifactPath}`
      : autostart.artifactPath;
  }, [autostart]);

  async function toggleAutostart() {
    if (!autostart) {
      return;
    }

    setWorking(true);
    setError(null);
    setSaved(false);
    try {
      setAutostart(await setAutostartEnabled(!autostart.enabled));
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  async function toggleDiagnostics(enabled: boolean) {
    setDiagnosticsWorking(true);
    setDiagnosticsError(null);
    try {
      setDiagnostics(await setDiagnosticsEnabled(enabled));
    } catch (error) {
      setDiagnosticsError(redactOperationalError(error));
    } finally {
      setDiagnosticsWorking(false);
    }
  }

  async function saveHotkeys() {
    setWorking(true);
    setError(null);
    setSaved(false);
    try {
      const status = await saveGlobalHotkeys(settings.map(toKeyEventPayload));
      setHotkeys(status);
      setSettings(status.settings.map(toMutableSetting));
      setSaved(true);
    } catch (error) {
      setError(getErrorMessage(error));
    } finally {
      setWorking(false);
    }
  }

  function patchSetting(action: GlobalHotkey, patch: Partial<MutableKeyEventItem>) {
    setSaved(false);
    setSettings((current) =>
      current.map((setting) => (setting.EGlobalHotkey === action ? { ...setting, ...patch } : setting)),
    );
  }

  return (
    <section className="grid gap-4">
      <Card className="gap-3 rounded-md bg-background p-3 shadow-none">
        <CardHeader className="p-0">
          <CardTitle className="flex items-center gap-2 text-sm">
            <Power className="size-4 text-muted-foreground" aria-hidden="true" />
            {t("options.autostart")}
          </CardTitle>
        </CardHeader>
        <CardContent className="grid gap-2 p-0">
          <div className="flex flex-wrap items-center gap-2">
            <Button
              disabled={!autostart || working}
              onClick={() => void toggleAutostart()}
              type="button"
              variant="outline"
            >
              {autostart?.enabled ? t("options.disableAutostart") : t("options.enableAutostart")}
            </Button>
            <span className="text-xs text-muted-foreground">
              {autostart?.enabled ? t("options.autostartOn") : t("options.autostartOff")}
            </span>
          </div>
          {artifact ? <p className="break-all text-xs text-muted-foreground">{artifact}</p> : null}
        </CardContent>
      </Card>

      <Card className="gap-3 rounded-md bg-background p-3 shadow-none">
        <CardHeader className="p-0">
          <CardTitle className="flex items-center gap-2 text-sm">
            <Activity className="size-4 text-muted-foreground" aria-hidden="true" />
            {t("options.diagnostics")}
          </CardTitle>
        </CardHeader>
        <CardContent className="grid gap-2 p-0">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="grid min-w-0 gap-1">
              <Label className="text-sm font-normal" htmlFor="diagnostics-enabled">
                {diagnostics?.enabled ? t("options.diagnosticsOn") : t("options.diagnosticsOff")}
              </Label>
              <p className="text-xs text-muted-foreground">{t("options.diagnosticsScope")}</p>
            </div>
            <Switch
              aria-label={t("options.diagnostics")}
              checked={diagnostics?.enabled ?? true}
              disabled={!diagnostics || diagnosticsWorking}
              id="diagnostics-enabled"
              onCheckedChange={(checked) => void toggleDiagnostics(checked)}
            />
          </div>
          {diagnosticsError ? <p className="text-xs text-destructive">{diagnosticsError}</p> : null}
        </CardContent>
      </Card>

      <Card className="gap-3 rounded-md bg-background p-3 shadow-none">
        <CardHeader className="p-0">
          <CardTitle className="flex items-center gap-2 text-sm">
            <Keyboard className="size-4 text-muted-foreground" aria-hidden="true" />
            {t("options.hotkeys")}
          </CardTitle>
        </CardHeader>
        <CardContent className="grid gap-3 p-0">
          <div className="grid gap-2">
            {hotkeys?.actions.map((action) => {
              const setting = settings.find((item) => item.EGlobalHotkey === action.action);

              if (!setting) {
                return null;
              }

              return (
                <div key={action.action} className="grid gap-2 rounded-md border bg-muted/30 p-2 text-sm">
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium">{hotkeyLabel(action.action, action.label, t)}</span>
                    <Button
                      className="h-7 px-2 text-xs"
                      onClick={() => patchSetting(action.action, { KeyCode: null })}
                      type="button"
                      variant="ghost"
                    >
                      {t("actions.clear")}
                    </Button>
                  </div>
                  <div className="grid gap-2 sm:grid-cols-[1fr_9rem]">
                    <div className="flex flex-wrap gap-2">
                      <ModifierButton
                        active={setting.Control}
                        label="Ctrl"
                        onClick={() => patchSetting(action.action, { Control: !setting.Control })}
                      />
                      <ModifierButton
                        active={setting.Alt}
                        label="Alt"
                        onClick={() => patchSetting(action.action, { Alt: !setting.Alt })}
                      />
                      <ModifierButton
                        active={setting.Shift}
                        label="Shift"
                        onClick={() => patchSetting(action.action, { Shift: !setting.Shift })}
                      />
                    </div>
                    <Input
                      aria-label={t("options.hotkeyKey")}
                      className="h-8 px-2 text-sm"
                      onKeyDown={(event) => {
                        const keyCode = keyCodeFromEvent(event);
                        if (keyCode !== null) {
                          patchSetting(action.action, { KeyCode: keyCode });
                        }
                      }}
                      readOnly
                      value={keyCodeLabel(setting.KeyCode)}
                    />
                  </div>
                </div>
              );
            })}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <Button disabled={!hotkeys || working} onClick={() => void saveHotkeys()} type="button" variant="outline">
              <Save className="size-4" aria-hidden="true" />
              {t("actions.save")}
            </Button>
            {saved ? <span className="text-xs text-muted-foreground">{t("options.saved")}</span> : null}
            {hotkeys ? (
              <span className="text-xs text-muted-foreground">
                {t("options.hotkeysRegistered", { count: hotkeys.registered.length })}
              </span>
            ) : null}
          </div>
        </CardContent>
      </Card>

      {error ? <p className="text-xs text-destructive">{error}</p> : null}
    </section>
  );
}

function ModifierButton({
  active,
  label,
  onClick,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      aria-pressed={active}
      className="h-8 px-2 text-xs"
      onClick={onClick}
      type="button"
      variant={active ? "secondary" : "outline"}
    >
      {label}
    </Button>
  );
}

function toMutableSetting(setting: KeyEventItem_Serialize): MutableKeyEventItem {
  return {
    Alt: setting.Alt,
    Control: setting.Control,
    EGlobalHotkey: setting.EGlobalHotkey,
    KeyCode: setting.KeyCode ?? null,
    Shift: setting.Shift,
  };
}

function toKeyEventPayload(setting: MutableKeyEventItem): KeyEventItem_Deserialize {
  return {
    Alt: setting.Alt,
    Control: setting.Control,
    EGlobalHotkey: setting.EGlobalHotkey,
    KeyCode: setting.KeyCode,
    Shift: setting.Shift,
  };
}

function hotkeyLabel(action: GlobalHotkey, fallback: string, t: (key: string) => string): string {
  switch (action) {
    case 0:
      return t("options.hotkeyShowWindow");
    case 1:
      return t("options.hotkeyProxyClear");
    case 2:
      return t("options.hotkeyProxySet");
    case 3:
      return t("options.hotkeyProxyKeep");
    case 4:
      return t("options.hotkeyProxyPac");
    default:
      return fallback;
  }
}

function keyCodeFromEvent(event: KeyboardEvent<HTMLInputElement>): number | null {
  if (["Alt", "Control", "Meta", "Shift"].includes(event.key)) {
    return null;
  }

  event.preventDefault();
  return event.keyCode || event.which || null;
}

function keyCodeLabel(keyCode: number | null): string {
  if (!keyCode) {
    return "";
  }
  if (keyCode >= 65 && keyCode <= 90) {
    return String.fromCharCode(keyCode);
  }
  if (keyCode >= 48 && keyCode <= 57) {
    return String.fromCharCode(keyCode);
  }
  if (keyCode >= 112 && keyCode <= 135) {
    return `F${keyCode - 111}`;
  }

  const labels: Record<number, string> = {
    8: "Backspace",
    9: "Tab",
    13: "Enter",
    27: "Esc",
    32: "Space",
    33: "Page Up",
    34: "Page Down",
    35: "End",
    36: "Home",
    37: "Left",
    38: "Up",
    39: "Right",
    40: "Down",
    45: "Insert",
    46: "Delete",
    186: ";",
    187: "=",
    188: ",",
    189: "-",
    190: ".",
    191: "/",
    192: "`",
    219: "[",
    220: "\\",
    221: "]",
    222: "'",
  };

  return labels[keyCode] ?? `#${keyCode}`;
}
