import { RefreshCw } from "lucide-react";

import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { Spinner } from "@voya/ui/components/spinner";
import { cn } from "@voya/ui/lib/utils";
import { formatTimeOfDay } from "@voya/utils/formatting";

import type { SelfHostEnvironmentReport, SelfHostFamilyReport, SelfHostState } from "@voya/contracts";

import {
  ADDRESS_KIND_KEYS,
  REACHABILITY_KEYS,
  REASON_KEYS,
  TONE_DOT_CLASS,
  VERDICT_KEYS,
  actionableReasons,
  overallReachability,
  reachabilityTone,
} from "./self-host-labels";
import type { SelfHostController } from "./use-self-host";

const SELF_TEST_FAILURES = [
  { key: "vless", sentence: "panes.selfHost.network.selfTest.vlessFailed" },
  { key: "shadowsocks", sentence: "panes.selfHost.network.selfTest.shadowsocksFailed" },
] as const satisfies readonly {
  key: keyof SelfHostEnvironmentReport["selfTest"];
  sentence: TranslationKey;
}[];

/**
 * Whether another device can reach the node, inside the hosting card: one
 * verdict, one line per address family, and only what needs doing. The backend
 * re-checks after the node starts and every ten minutes, so this stays current
 * without the button, which is offered only while hosting is on.
 */
export function NetworkStatus({
  controller,
  state,
}: {
  controller: SelfHostController;
  state: SelfHostState;
}) {
  const { t } = useI18n();
  const { environment } = state;
  const checking = controller.pending === "check";
  const verdict = environment ? overallReachability(environment) : null;

  return (
    <div className="grid gap-3 border-t border-border-subtle pt-4" data-testid="self-host-network">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2" data-testid="self-host-verdict">
        {verdict ? (
          <span
            aria-hidden="true"
            className={cn("size-2 shrink-0 rounded-full", TONE_DOT_CLASS[reachabilityTone(verdict)])}
          />
        ) : null}
        <span aria-live="polite" className={cn("text-sm", verdict ? "font-medium" : "text-muted-foreground")}>
          {verdict ? t(VERDICT_KEYS[verdict]) : t("panes.selfHost.network.notChecked")}
        </span>
        <span className="ms-auto flex items-center gap-3">
          {environment?.checkedAtMs ? (
            <span className="text-xs text-muted-foreground">
              {t("panes.selfHost.network.checkedAt", { time: formatTimeOfDay(environment.checkedAtMs) })}
            </span>
          ) : null}
          {state.config.enabled ? (
            <Button
              disabled={checking}
              onClick={() => void controller.runCheck()}
              size="sm"
              type="button"
              variant="outline"
            >
              {checking ? (
                <Spinner aria-hidden="true" className="size-4" />
              ) : (
                <RefreshCw aria-hidden="true" className="size-4" />
              )}
              {checking
                ? t("panes.selfHost.network.checking")
                : environment
                  ? t("panes.selfHost.network.run")
                  : t("panes.selfHost.network.title")}
            </Button>
          ) : null}
        </span>
      </div>
      {environment && verdict ? (
        <Findings
          controller={controller}
          environment={environment}
          firewallRuleSupported={state.firewallRuleSupported}
          needsForwarding={verdict === "needsPortForward" || verdict === "unreachable"}
          reachable={verdict === "reachable"}
        />
      ) : null}
    </div>
  );
}

function Findings({
  controller,
  environment,
  firewallRuleSupported,
  needsForwarding,
  reachable,
}: {
  controller: SelfHostController;
  environment: SelfHostEnvironmentReport;
  firewallRuleSupported: boolean;
  needsForwarding: boolean;
  reachable: boolean;
}) {
  const { t } = useI18n();
  // A peer that can connect needs no advice; one that cannot gets every step.
  const reasons = reachable ? [] : actionableReasons(environment);
  const failures = SELF_TEST_FAILURES.filter(({ key }) => environment.selfTest[key] === "failed");
  // Where the router has to forward to; only the private IPv4 address is.
  const localAddresses = needsForwarding
    ? environment.localAddresses
        .filter((entry) => entry.family === "ipv4" && entry.scope === "private")
        .map((entry) => entry.address)
    : [];

  return (
    <>
      <div className="grid gap-1.5">
        <FamilyRow report={environment.ipv4} />
        <FamilyRow report={environment.ipv6} />
      </div>
      {failures.length > 0 ? (
        <ul className="grid gap-1 text-sm text-danger" data-testid="self-host-self-test">
          {failures.map(({ sentence }) => (
            <li key={sentence}>{t(sentence)}</li>
          ))}
        </ul>
      ) : null}
      {reasons.length > 0 ? (
        <ul className="grid list-disc gap-1 ps-5 text-sm text-muted-foreground">
          {reasons.map((reason) => (
            <li key={reason}>{t(REASON_KEYS[reason])}</li>
          ))}
        </ul>
      ) : null}
      {firewallRuleSupported && environment.firewall !== "rulePresent" ? (
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <p className="min-w-0 flex-1 text-sm text-muted-foreground">
            {t("panes.selfHost.network.reasons.firewallRuleMissing")}
          </p>
          <Button
            disabled={controller.pending === "firewall"}
            onClick={() => void controller.applyFirewallRule()}
            size="sm"
            type="button"
            variant="outline"
          >
            {t("panes.selfHost.network.firewall.allow")}
          </Button>
        </div>
      ) : null}
      {localAddresses.length > 0 ? (
        <p className="text-xs text-muted-foreground">
          {t("panes.selfHost.network.localAddress", { addresses: localAddresses.join(", ") })}
        </p>
      ) : null}
      {reachable ? (
        <p className="text-xs text-muted-foreground">{t("panes.selfHost.network.probeNote")}</p>
      ) : null}
    </>
  );
}

function FamilyRow({ report }: { report: SelfHostFamilyReport }) {
  const { t } = useI18n();
  return (
    <div className="flex min-w-0 items-center gap-3 text-sm" data-testid={`self-host-family-${report.family}`}>
      <span className="w-10 shrink-0 text-muted-foreground">{t(ADDRESS_KIND_KEYS[report.family])}</span>
      <span className="min-w-0 truncate font-mono">
        {report.publicAddress ?? t("panes.selfHost.network.none")}
      </span>
      <Badge className="ms-auto" variant={reachabilityTone(report.reachability)}>
        {t(REACHABILITY_KEYS[report.reachability])}
      </Badge>
    </div>
  );
}
