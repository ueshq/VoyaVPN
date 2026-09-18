import { ShieldCheck } from "lucide-react";

import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { formatTimeOfDay } from "@voya/utils/formatting";

import { SettingsGroup, SettingsRow } from "@/features/settings/settings-form";
import type { SelfHostFamilyReport, SelfHostSelfTest, SelfHostState } from "@/ipc/bindings";

import {
  ADDRESS_KIND_KEYS,
  FIREWALL_KEYS,
  NAT_KEYS,
  PORT_MAPPING_KEYS,
  REACHABILITY_KEYS,
  REASON_KEYS,
  SELF_TEST_KEYS,
  reachabilityTone,
  selfTestTone,
} from "./self-host-labels";
import type { SelfHostController } from "./use-self-host";

/**
 * Where this device sits between the node and the internet, one row per
 * address family, with every finding spelled out as what it means and what to
 * do about it.
 */
export function EnvironmentCard({
  controller,
  state,
}: {
  controller: SelfHostController;
  state: SelfHostState;
}) {
  const { t } = useI18n();
  const environment = state.environment;

  return (
    <SettingsGroup
      actions={
        environment?.checkedAtMs ? (
          <span className="text-xs text-muted-foreground">
            {t("panes.selfHost.network.checkedAt", {
              time: formatTimeOfDay(environment.checkedAtMs),
            })}
          </span>
        ) : null
      }
      title={t("panes.selfHost.network.title")}
    >
      {environment === null ? (
        <p className="text-sm text-muted-foreground">{t("panes.selfHost.network.never")}</p>
      ) : (
        <>
          <SelfTestRow selfTest={environment.selfTest} />
          <FamilyRow label={t(ADDRESS_KIND_KEYS.ipv4)} report={environment.ipv4} />
          <FamilyRow label={t(ADDRESS_KIND_KEYS.ipv6)} report={environment.ipv6} />
          <SettingsRow label={t("panes.selfHost.network.upnp.title")}>
            <span className="text-sm">
              {t(PORT_MAPPING_KEYS[environment.portMapping.status], {
                ports: environment.portMapping.mappedPorts.join(", "),
              })}
            </span>
          </SettingsRow>
          {state.firewallRuleSupported ? (
            <SettingsRow label={t("panes.selfHost.network.firewall.title")}>
              <div className="flex flex-wrap items-center justify-end gap-2">
                <span className="text-sm">{t(FIREWALL_KEYS[environment.firewall])}</span>
                {environment.firewall === "rulePresent" ? null : (
                  <Button
                    disabled={controller.pending === "firewall"}
                    onClick={() => void controller.applyFirewallRule()}
                    size="sm"
                    type="button"
                    variant="outline"
                  >
                    <ShieldCheck aria-hidden="true" className="size-4" />
                    {t("panes.selfHost.network.firewall.allow")}
                  </Button>
                )}
              </div>
            </SettingsRow>
          ) : null}
          {environment.localAddresses.length > 0 ? (
            <SettingsRow label={t("panes.selfHost.network.localAddresses")}>
              <ul className="grid justify-items-end gap-0.5 font-mono text-xs text-muted-foreground">
                {environment.localAddresses.map((entry) => (
                  <li key={`${entry.interface}-${entry.address}`}>
                    {`${entry.interface} ${entry.address}`}
                  </li>
                ))}
              </ul>
            </SettingsRow>
          ) : null}
          <p className="text-xs text-muted-foreground">{t("panes.selfHost.network.probeNote")}</p>
        </>
      )}
    </SettingsGroup>
  );
}

/**
 * A client on this device signed in to the node and fetched a page through it.
 * The one check that catches a disguise site REALITY cannot use.
 */
function SelfTestRow({ selfTest }: { selfTest: SelfHostSelfTest }) {
  const { t } = useI18n();
  const protocols = [
    {
      failureKey: "panes.selfHost.network.selfTest.vlessFailed",
      labelKey: "panes.selfHost.config.vless",
      result: selfTest.vless,
    },
    {
      failureKey: "panes.selfHost.network.selfTest.shadowsocksFailed",
      labelKey: "panes.selfHost.config.shadowsocks",
      result: selfTest.shadowsocks,
    },
  ] as const satisfies readonly {
    failureKey: TranslationKey;
    labelKey: TranslationKey;
    result: SelfHostSelfTest["vless"];
  }[];
  if (protocols.every(({ result }) => result === "skipped")) return null;
  return (
    <div className="grid gap-2" data-testid="self-host-self-test">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span className="text-sm font-medium">{t("panes.selfHost.network.selfTest.title")}</span>
        <div className="ms-auto flex flex-wrap items-center gap-2">
          {protocols.map(({ labelKey, result }) => (
            <Badge key={labelKey} variant={selfTestTone(result)}>
              {`${t(labelKey)} · ${t(SELF_TEST_KEYS[result])}`}
            </Badge>
          ))}
        </div>
      </div>
      {protocols.some(({ result }) => result === "failed") ? (
        <ul className="grid list-disc gap-1 ps-5 text-sm text-danger">
          {protocols
            .filter(({ result }) => result === "failed")
            .map(({ failureKey }) => (
              <li key={failureKey}>{t(failureKey)}</li>
            ))}
        </ul>
      ) : null}
    </div>
  );
}

function FamilyRow({ label, report }: { label: string; report: SelfHostFamilyReport }) {
  const { t } = useI18n();
  return (
    <div className="grid gap-2" data-testid={`self-host-family-${report.family}`}>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span className="text-sm font-medium">{label}</span>
        <span className="font-mono text-sm">
          {report.publicAddress ?? t("panes.selfHost.network.none")}
        </span>
        <span className="text-sm text-muted-foreground">{t(NAT_KEYS[report.nat])}</span>
        <Badge className="ms-auto" variant={reachabilityTone(report.reachability)}>
          {t(REACHABILITY_KEYS[report.reachability])}
        </Badge>
      </div>
      <p className="text-xs text-muted-foreground">
        {t(report.verifiedByProbe ? "panes.selfHost.network.verified" : "panes.selfHost.network.inferred")}
      </p>
      {report.reasons.length > 0 ? (
        <ul className="grid list-disc gap-1 ps-5 text-sm">
          {report.reasons.map((reason) => (
            <li key={reason}>{t(REASON_KEYS[reason])}</li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
