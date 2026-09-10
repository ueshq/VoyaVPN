import { SettingsCheckbox, NumberField, SelectField, TextField, SettingsCheckboxGroup, SettingsGroup, SettingsRow } from "./settings-form";
import { useState } from "react";
import { AppWindow } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { Separator } from "@voya/ui/components/separator";
import { useI18n } from "@voya/i18n/use-i18n";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";
import { PerAppProxyDialog } from "@/features/routing/per-app-proxy-dialog";

import { TunDiagnosticsButton } from "./tun-diagnostics-button";
import type { AppSettingsController } from "./use-app-settings";

// Both are closed sets in sing-box, and neither is validated on save: an unknown
// stack only fails when the core is next started, and an unknown ICMP policy is
// silently coerced to "rule". Free text could therefore change behaviour with no
// feedback at all, so the UI offers exactly the accepted values.
const TUN_STACKS = ["system", "gvisor", "mixed"];
const TUN_ICMP_ROUTING = ["rule", "direct", "unreachable", "drop", "reply"];
// What the generator falls back to when the stored value is empty; showing it
// keeps the control honest about what the core will actually use.
const DEFAULT_TUN_STACK = "gvisor";
const DEFAULT_TUN_ICMP_ROUTING = "rule";

export function NetworkTab({ controller }: { controller: AppSettingsController }) {
  const { t } = useI18n();
  const { settings, error, update, working } = controller;
  const [perAppOpen, setPerAppOpen] = useState(false);
  const proxyManagement = useRuntimeEventStore((state) => state.sysProxy?.management);

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
          <SettingsCheckbox checked={settings.network.tun.autoRoute} label={t("settings.network.tunAutoRoute")} onCheckedChange={(autoRoute) => patchTun({ autoRoute: autoRoute === true })} />
          <SettingsCheckbox checked={settings.network.tun.strictRoute} label={t("settings.network.tunStrictRoute")} onCheckedChange={(strictRoute) => patchTun({ strictRoute: strictRoute === true })} />
          <SettingsCheckbox checked={settings.network.tun.ipv6Enabled} label={t("settings.network.enableIpv6Address")} onCheckedChange={(ipv6Enabled) => patchTun({ ipv6Enabled: ipv6Enabled === true })} />
        </SettingsCheckboxGroup>
        <SelectField
          id="rt-tun-stack"
          label={t("settings.network.tunStack")}
          onChange={(stack) => patchTun({ stack })}
          options={TUN_STACKS}
          value={settings.network.tun.stack || DEFAULT_TUN_STACK}
        />
        <NumberField id="rt-tun-mtu" label={t("settings.network.mtu")} onChange={(mtu) => patchTun({ mtu: mtu ?? 1500 })} value={settings.network.tun.mtu} />
        <SelectField
          id="rt-tun-icmp-routing"
          label={t("settings.network.icmpRoutingPolicy")}
          onChange={(icmpRouting) => patchTun({ icmpRouting })}
          options={TUN_ICMP_ROUTING}
          value={settings.network.tun.icmpRouting || DEFAULT_TUN_ICMP_ROUTING}
        />
        <TunDiagnosticsButton />
      </SettingsGroup>

      <Separator />

      <SettingsGroup>
        <SettingsRow>
          <SettingsCheckbox checked={settings.network.systemProxy.bypassLocal} label={t("settings.network.bypassLocalAddress")} onCheckedChange={(bypassLocal) => patchSystemProxy({ bypassLocal: bypassLocal === true })} />
        </SettingsRow>
        <TextField id="rt-sysproxy-exceptions" label={t("settings.network.systemProxyExceptions")} onChange={(exceptions) => patchSystemProxy({ exceptions })} value={settings.network.systemProxy.exceptions} />
        <TextField id="rt-sysproxy-advanced-protocol" label={t("settings.network.systemProxyProtocol")} onChange={(advancedProtocol) => patchSystemProxy({ advancedProtocol })} value={settings.network.systemProxy.advancedProtocol} />
        <TextField id="rt-sysproxy-pac-path" label={t("settings.network.customPacPath")} onChange={(value) => patchSystemProxy({ customPacPath: nullableText(value) })} value={settings.network.systemProxy.customPacPath ?? ""} />
        {proxyManagement === "automatic" ? <TextField id="rt-sysproxy-script-path" label={t("settings.network.customScriptPath")} onChange={(value) => patchSystemProxy({ customScriptPath: nullableText(value) })} value={settings.network.systemProxy.customScriptPath ?? ""} /> : null}
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
