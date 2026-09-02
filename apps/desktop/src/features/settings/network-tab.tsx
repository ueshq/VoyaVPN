import { useState } from "react";
import { AppWindow } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { Separator } from "@voya/ui/components/separator";
import { useI18n } from "@voya/i18n/use-i18n";
import { PerAppProxyDialog } from "@/features/routing/per-app-proxy-dialog";

import { CheckboxField, NumberField, TextField } from "./runtime-fields";
import { SettingsCheckboxGroup, SettingsGroup, SettingsRow } from "./settings-form";
import { TunDiagnosticsButton } from "./tun-diagnostics-button";
import type { AppSettingsController } from "./use-app-settings";

export function NetworkTab({ controller }: { controller: AppSettingsController }) {
  const { t } = useI18n();
  const { settings, error, update, working } = controller;
  const [perAppOpen, setPerAppOpen] = useState(false);

  if (!settings) {
    return <p className="text-xs text-muted-foreground">{working ? t("options.loading") : error}</p>;
  }

  const patchTun = (patch: Partial<typeof settings.network.tun>) =>
    update((current) => ({
      ...current,
      network: {
        ...current.network,
        tun: { ...current.network.tun, ...patch },
      },
    }));
  const patchSystemProxy = (patch: Partial<typeof settings.network.systemProxy>) =>
    update((current) => ({
      ...current,
      network: {
        ...current.network,
        systemProxy: { ...current.network.systemProxy, ...patch },
      },
    }));

  return (
    <div className="grid gap-4">
      <SettingsGroup>
        <SettingsCheckboxGroup id="rt-tun-group" label={t("settings.network.tunMode")}>
          <CheckboxField checked={settings.network.tun.autoRoute} label={t("settings.network.tunAutoRoute")} onChange={(autoRoute) => patchTun({ autoRoute })} />
          <CheckboxField checked={settings.network.tun.strictRoute} label={t("settings.network.tunStrictRoute")} onChange={(strictRoute) => patchTun({ strictRoute })} />
          <CheckboxField checked={settings.network.tun.ipv6Enabled} label={t("settings.network.enableIpv6Address")} onChange={(ipv6Enabled) => patchTun({ ipv6Enabled })} />
        </SettingsCheckboxGroup>
        <TextField id="rt-tun-stack" label={t("settings.network.tunStack")} onChange={(stack) => patchTun({ stack })} value={settings.network.tun.stack} />
        <NumberField id="rt-tun-mtu" label={t("settings.network.mtu")} onChange={(mtu) => patchTun({ mtu: mtu ?? 1500 })} value={settings.network.tun.mtu} />
        <TextField id="rt-tun-icmp-routing" label={t("settings.network.icmpRoutingPolicy")} onChange={(icmpRouting) => patchTun({ icmpRouting })} value={settings.network.tun.icmpRouting} />
        <TunDiagnosticsButton />
      </SettingsGroup>

      <Separator />

      <SettingsGroup>
        <SettingsRow>
          <CheckboxField checked={settings.network.systemProxy.bypassLocal} label={t("settings.network.bypassLocalAddress")} onChange={(bypassLocal) => patchSystemProxy({ bypassLocal })} />
        </SettingsRow>
        <TextField id="rt-sysproxy-exceptions" label={t("settings.network.systemProxyExceptions")} onChange={(exceptions) => patchSystemProxy({ exceptions })} value={settings.network.systemProxy.exceptions} />
        <TextField id="rt-sysproxy-advanced-protocol" label={t("settings.network.systemProxyProtocol")} onChange={(advancedProtocol) => patchSystemProxy({ advancedProtocol })} value={settings.network.systemProxy.advancedProtocol} />
        <TextField id="rt-sysproxy-pac-path" label={t("settings.network.customPacPath")} onChange={(value) => patchSystemProxy({ customPacPath: nullableText(value) })} value={settings.network.systemProxy.customPacPath ?? ""} />
        <TextField id="rt-sysproxy-script-path" label={t("settings.network.customScriptPath")} onChange={(value) => patchSystemProxy({ customScriptPath: nullableText(value) })} value={settings.network.systemProxy.customScriptPath ?? ""} />
      </SettingsGroup>

      <Separator />

      {/* Immediate action, deliberately outside the draft/Save-all model: the
          editor writes routing rules directly through its own dialog. */}
      <SettingsGroup>
        <SettingsRow description={t("panes.routing.perAppDescription")} label={t("settings.perAppProxy")}>
          <Button onClick={() => setPerAppOpen(true)} size="sm" type="button" variant="outline">
            <AppWindow aria-hidden="true" className="size-4" />
            {t("actions.edit")}
          </Button>
        </SettingsRow>
      </SettingsGroup>

      {perAppOpen ? <PerAppProxyDialog onOpenChange={setPerAppOpen} open={perAppOpen} /> : null}
    </div>
  );
}

function nullableText(value: string): string | null {
  return value.trim() ? value : null;
}
