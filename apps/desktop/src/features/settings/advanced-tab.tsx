import { useEffect, useRef, useState } from "react";

import { Disclosure } from "@voya/ui/components/disclosure";
import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { LogsPanel, type LogFilter } from "@/features/logs/logs-panel";
import { useRuntimeEventStore } from "@/ipc/runtime-event-store";

import { useShellStore } from "@/stores/shell-store";
import { CoreTab } from "./core-tab";
import {
  NumberField,
  SelectField,
  SettingsGroup,
  SettingsSwitch,
  TextField,
} from "./settings-form";
import { SETTING_DEFAULTS } from "./settings-values";
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
const TUN_STACK_LABELS: Record<string, TranslationKey> = {
  gvisor: "settings.network.tunStackOptions.gvisor",
  mixed: "settings.network.tunStackOptions.mixed",
  system: "settings.network.tunStackOptions.system",
};
const TUN_ICMP_LABELS: Record<string, TranslationKey> = {
  direct: "settings.network.icmpOptions.direct",
  drop: "settings.network.icmpOptions.drop",
  reply: "settings.network.icmpOptions.reply",
  rule: "settings.network.icmpOptions.rule",
  unreachable: "settings.network.icmpOptions.unreachable",
};

/**
 * Everything that needs networking knowledge: tunnel parameters, the
 * system proxy, the core and the runtime log. Speed test settings
 * sit with the tests on the Nodes page.
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
      <SettingsGroup
        title={t("settings.sections.tun")}
        actions={<TunDiagnosticsButton />}
      >
        <SettingsSwitch
          field="network.tun.autoRoute"
          checked={settings.network.tun.autoRoute}
          description={t("settings.network.tunAutoRouteHint")}
          label={t("settings.network.tunAutoRoute")}
          onCheckedChange={(autoRoute) =>
            patchTun({ autoRoute })
          }
        />
        <SettingsSwitch
          field="network.tun.ipv6Enabled"
          checked={settings.network.tun.ipv6Enabled}
          description={t("settings.network.enableIpv6Hint")}
          label={t("settings.network.enableIpv6Address")}
          onCheckedChange={(ipv6Enabled) =>
            patchTun({ ipv6Enabled })
          }
        />
        <Disclosure
          title={t("common.advanced")}
          invalid={Object.keys(controller.fieldErrors).some((field) =>
            field.startsWith("network.tun"),
          )}
        >
          <SelectField
            description={t("settings.network.tunStackHint")}
            field="network.tun.stack"
            id="rt-tun-stack"
            label={t("settings.network.tunStack")}
            onChange={(stack) => patchTun({ stack })}
            optionLabel={TUN_STACK_LABELS}
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
            description={t("settings.network.icmpHint")}
            field="network.tun.icmpRouting"
            id="rt-tun-icmp-routing"
            label={t("settings.network.icmpRoutingPolicy")}
            onChange={(icmpRouting) => patchTun({ icmpRouting })}
            optionLabel={TUN_ICMP_LABELS}
            options={TUN_ICMP_ROUTING}
            value={settings.network.tun.icmpRouting || DEFAULT_TUN_ICMP_ROUTING}
          />
        </Disclosure>
      </SettingsGroup>

      {systemProxyManaged ? (
        <SettingsGroup title={t("settings.sections.systemProxy")}>
          <SettingsSwitch
            field="network.systemProxy.bypassLocal"
            checked={settings.network.systemProxy.bypassLocal}
            label={t("settings.network.bypassLocalAddress")}
            onCheckedChange={(bypassLocal) =>
              patchSystemProxy({ bypassLocal })
            }
          />
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
      <RuntimeLogGroup
        coreLogEnabled={settings.core.logEnabled}
        logLevel={settings.core.logLevel}
      />
    </div>
  );
}

/** The runtime log, kept with the other diagnostics rather than a main page. */
function RuntimeLogGroup({
  coreLogEnabled,
  logLevel,
}: {
  coreLogEnabled: boolean;
  logLevel: string;
}) {
  const { t } = useI18n();
  const [search, setSearch] = useState("");
  const headingRef = useRef<HTMLHeadingElement>(null);
  const target = useShellStore((state) => state.settingsTarget);
  useEffect(() => {
    if (target !== "logs") return;
    const frame = requestAnimationFrame(() => {
      const heading = headingRef.current;
      if (!heading) return;
      heading.focus({ preventScroll: true });
      heading.scrollIntoView({ block: "start" });
      heading.closest("section")?.setAttribute("data-settings-highlight", "true");
      useShellStore.getState().consumeSettingsTarget();
    });
    return () => cancelAnimationFrame(frame);
  }, [target]);
  const [filter, setFilter] = useState<LogFilter>("standard");
  const levelHint = coreLogEnabled ? logLevelHint(logLevel, filter) : null;

  return (
    <SettingsGroup headingRef={headingRef} title={t("tabs.logs")}>
      {coreLogEnabled ? null : (
        <p className="text-xs text-muted-foreground">
          {t("settings.logs.coreLogOff")}
        </p>
      )}
      {levelHint ? (
        <p className="text-xs text-muted-foreground" role="status">
          {t(levelHint)}
        </p>
      ) : null}
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

/**
 * The level decides what the connection records and the filter what is shown.
 * When they disagree, the hint names the control to change.
 */
function logLevelHint(level: string, filter: LogFilter): TranslationKey | null {
  if ((level === "debug" || level === "trace") && filter !== "all") {
    return "panes.logs.debugHidden";
  }
  if ((level === "warn" || level === "error") && filter !== "issues") {
    return "panes.logs.infoNotRecorded";
  }
  return null;
}
