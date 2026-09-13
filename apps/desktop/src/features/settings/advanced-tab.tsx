import { useState } from "react";

import { Disclosure } from "@voya/ui/components/disclosure";
import { useI18n } from "@voya/i18n/use-i18n";
import { LogsPanel, type LogFilter } from "@/features/logs/logs-panel";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";

import { CaptureModeSetting } from "./capture-mode-setting";
import { CoreTab } from "./core-tab";
import {
  NumberField,
  SelectField,
  SettingsCheckbox,
  SettingsGroup,
  SettingsRow,
  TextField,
} from "./settings-form";
import { SETTING_DEFAULTS } from "./settings-values";
import { TestsTab } from "./tests-tab";
import { TunDiagnosticsButton } from "./tun-diagnostics-button";
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

/**
 * Everything that needs networking knowledge: how traffic is captured, the
 * tunnel, the system proxy, the core, node tests and the runtime log.
 */
export function AdvancedTab({
  controller,
}: {
  controller: AppSettingsFormController;
}) {
  const { t } = useI18n();
  const { settings, update } = controller;
  // macOS has no system proxy mode, so its group is only offered where the
  // app manages one.
  const systemProxyManaged = useRuntimeEventStore(
    (state) => state.sysProxy?.management === "automatic",
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
      <CaptureModeSetting />

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
            field="network.tun.ipv6Enabled"
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
            defaultValue={SETTING_DEFAULTS.tunMtu}
            field="network.tun.mtu"
            id="rt-tun-mtu"
            label={t("settings.network.mtu")}
            onChange={(mtu) => patchTun({ mtu: mtu ?? SETTING_DEFAULTS.tunMtu })}
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

      {systemProxyManaged ? (
        <SettingsGroup title={t("settings.sections.systemProxy")}>
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
        </SettingsGroup>
      ) : null}

      <CoreTab controller={controller} />
      <TestsTab controller={controller} />
      <RuntimeLogGroup coreLogEnabled={settings.core.logEnabled} />
    </div>
  );
}

/** The runtime log, kept with the other diagnostics rather than a main page. */
function RuntimeLogGroup({ coreLogEnabled }: { coreLogEnabled: boolean }) {
  const { t } = useI18n();
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<LogFilter>("standard");

  return (
    <SettingsGroup title={t("tabs.logs")}>
      {coreLogEnabled ? null : (
        <p className="text-xs text-muted-foreground">
          {t("settings.logs.coreLogOff")}
        </p>
      )}
      <div className="h-[28rem] min-h-0 overflow-hidden rounded-md border">
        <LogsPanel
          filter={filter}
          onFilterChange={setFilter}
          onSearchChange={setSearch}
          search={search}
        />
      </div>
    </SettingsGroup>
  );
}
