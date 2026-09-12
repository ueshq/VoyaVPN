import { nullableText } from "./settings-values";
import {
  SettingsCheckbox,
  NumberField,
  SelectField,
  TextField,
  SettingsGroup,
  SettingsRow,
} from "./settings-form";
import { Disclosure } from "@voya/ui/components/disclosure";
import { useShellStore } from "@/stores/shell-store";
import { AppWindow } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { useI18n } from "@voya/i18n/use-i18n";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";

import { TunDiagnosticsButton } from "./tun-diagnostics-button";
import { ManualProxyPanel } from "./manual-proxy-panel";
import type { AppSettingsFormController } from "./use-app-settings";

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

export function NetworkTab({
  controller,
}: {
  controller: AppSettingsFormController;
}) {
  const { t } = useI18n();
  const { settings, update } = controller;
  const sysProxy = useRuntimeEventStore((state) => state.sysProxy);
  const connected = useRuntimeEventStore(
    (state) => state.coreState?.state === "connected",
  );
  const tunEnabled = useRuntimeEventStore(
    (state) => state.tun?.enabled ?? false,
  );

  const patchTun = (patch: Partial<typeof settings.network.tun>) =>
    update((current) => ({
      ...current,
      network: {
        ...current.network,
        tun: { ...current.network.tun, ...patch },
      },
    }));
  const patchSystemProxy = (
    patch: Partial<typeof settings.network.systemProxy>,
  ) =>
    update((current) => ({
      ...current,
      network: {
        ...current.network,
        systemProxy: { ...current.network.systemProxy, ...patch },
      },
    }));

  return (
    <div className="grid gap-4">
      <SettingsGroup
        title={t("settings.sections.tun")}
        actions={<TunDiagnosticsButton />}
      >
        <div className="grid gap-3 @min-[42rem]:grid-cols-2">
          <SettingsCheckbox
            field="network.tun.autoRoute"
            checked={settings.network.tun.autoRoute}
            label={t("settings.network.tunAutoRoute")}
            onCheckedChange={(autoRoute) =>
              patchTun({ autoRoute: autoRoute === true })
            }
          />
          <SettingsCheckbox
            field="network.tun.strictRoute"
            checked={settings.network.tun.strictRoute}
            label={t("settings.network.tunStrictRoute")}
            onCheckedChange={(strictRoute) =>
              patchTun({ strictRoute: strictRoute === true })
            }
          />
          <SettingsCheckbox
            field="network.tun.ipv"
            checked={settings.network.tun.ipv6Enabled}
            label={t("settings.network.enableIpv6Address")}
            onCheckedChange={(ipv6Enabled) =>
              patchTun({ ipv6Enabled: ipv6Enabled === true })
            }
          />
        </div>
        <Disclosure
          className="border-0 [&>div]:border-0"
          title={t("common.advanced")}
          invalid={Object.keys(controller.fieldErrors).some((field) =>
            field.startsWith("network.tun"),
          )}
        >
          <SelectField
            field="network.tun.stack"
            id="rt-tun-stack"
            label={t("settings.network.tunStack")}
            onChange={(stack) => patchTun({ stack })}
            options={TUN_STACKS}
            value={settings.network.tun.stack || DEFAULT_TUN_STACK}
          />
          <NumberField
            field="network.tun.mtu"
            id="rt-tun-mtu"
            label={t("settings.network.mtu")}
            onChange={(mtu) => patchTun({ mtu: mtu ?? 1500 })}
            value={settings.network.tun.mtu}
          />
          <SelectField
            field="network.tun.icmpRouting"
            id="rt-tun-icmp-routing"
            label={t("settings.network.icmpRoutingPolicy")}
            onChange={(icmpRouting) => patchTun({ icmpRouting })}
            options={TUN_ICMP_ROUTING}
            value={settings.network.tun.icmpRouting || DEFAULT_TUN_ICMP_ROUTING}
          />
        </Disclosure>
      </SettingsGroup>

      <SettingsGroup title={t("settings.sections.systemProxy")}>
        {sysProxy?.management === "manual" ? (
          <ManualProxyPanel
            status={sysProxy}
            connected={connected}
            tunEnabled={tunEnabled}
          />
        ) : null}
        <SettingsRow>
          <SettingsCheckbox
            field="network.systemProxy.bypassLocal"
            checked={settings.network.systemProxy.bypassLocal}
            label={t("settings.network.bypassLocalAddress")}
            onCheckedChange={(bypassLocal) =>
              patchSystemProxy({ bypassLocal: bypassLocal === true })
            }
          />
        </SettingsRow>
        <TextField
          field="network.systemProxy.exceptions"
          id="rt-sysproxy-exceptions"
          label={t("settings.network.systemProxyExceptions")}
          onChange={(exceptions) => patchSystemProxy({ exceptions })}
          value={settings.network.systemProxy.exceptions}
        />
        <Disclosure
          className="border-0 [&>div]:border-0"
          title={t("common.advanced")}
          invalid={Object.keys(controller.fieldErrors).some((field) =>
            field.startsWith("network.systemProxy"),
          )}
        >
          <TextField
            field="network.systemProxy.advancedProtocol"
            id="rt-sysproxy-advanced-protocol"
            label={t("settings.network.systemProxyProtocol")}
            onChange={(advancedProtocol) =>
              patchSystemProxy({ advancedProtocol })
            }
            value={settings.network.systemProxy.advancedProtocol}
          />
          <TextField
            field="network.systemProxy.customPacPath"
            id="rt-sysproxy-pac-path"
            label={t("settings.network.customPacPath")}
            onChange={(value) =>
              patchSystemProxy({ customPacPath: nullableText(value) })
            }
            value={settings.network.systemProxy.customPacPath ?? ""}
          />
          {sysProxy?.management === "automatic" ? (
            <TextField
              field="network.systemProxy.customScriptPath"
              id="rt-sysproxy-script-path"
              label={t("settings.network.customScriptPath")}
              onChange={(value) =>
                patchSystemProxy({ customScriptPath: nullableText(value) })
              }
              value={settings.network.systemProxy.customScriptPath ?? ""}
            />
          ) : null}
        </Disclosure>
      </SettingsGroup>

      {/* Explicit action: the
          editor writes routing rules directly through its own dialog. */}
      <SettingsGroup title={t("settings.perAppProxy")}>
        <SettingsRow
          description={t("panes.routing.perAppDescription")}
          label={t("settings.perAppProxy")}
        >
          <Button
            onClick={() => {
              useShellStore.setState({ routingPerAppRequested: true });
              useShellStore.getState().setActiveTab("rules", true);
            }}
            size="sm"
            type="button"
            variant="outline"
          >
            <AppWindow aria-hidden="true" className="size-4" />
            {t("settings.openRules")}
          </Button>
        </SettingsRow>
      </SettingsGroup>
    </div>
  );
}
