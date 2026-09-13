import { useState } from "react";

import type { TranslationKey } from "@voya/i18n";
import { useI18n } from "@voya/i18n/use-i18n";
import { SelectField } from "@voya/ui/components/form-fields";
import { Label } from "@voya/ui/components/label";
import { Switch } from "@voya/ui/components/switch";

import { PageSurface } from "@/components/app-shell/page-section";
import { SettingsApplyStatus } from "@/features/settings/settings-apply-status";
import { useAppSettings } from "@/features/settings/use-app-settings";

import { setQuickRule } from "./quick-rules";
import { DOMAIN_STRATEGIES } from "./routing-constants";
import { isQuickRuleEnabled, type QuickRule } from "./sentinel-rules";
import type { RoutingScreenController } from "./use-routing-screen";

type DomainStrategy = (typeof DOMAIN_STRATEGIES)[number];

const STRATEGY_LABELS: Record<DomainStrategy, TranslationKey> = {
  AsIs: "panes.routing.resolveAsIs",
  IPIfNonMatch: "panes.routing.resolveIpIfNonMatch",
  IPOnDemand: "panes.routing.resolveIpOnDemand",
};

const QUICK_RULES: ReadonlyArray<{
  hintKey: TranslationKey;
  labelKey: TranslationKey;
  rule: QuickRule;
}> = [
  { hintKey: "panes.routing.blockAdsHint", labelKey: "panes.routing.blockAds", rule: "blockAds" },
  { hintKey: "panes.routing.bypassLanHint", labelKey: "panes.routing.bypassLan", rule: "bypassLan" },
];

/**
 * One-click routing controls. The switches edit the managed rules of the
 * active routing profile, which is the one the core runs with; the IP
 * resolution mode is a global setting and goes through the settings save and
 * apply flow like every other setting.
 */
export function RoutingQuickSettings({
  controller,
}: {
  controller: RoutingScreenController;
}) {
  const { t } = useI18n();
  const appSettings = useAppSettings();
  const [pending, setPending] = useState<QuickRule | null>(null);
  const activeRouting =
    controller.routings.find((routing) => routing.isActive) ?? null;

  async function toggle(rule: QuickRule, enabled: boolean) {
    if (!activeRouting || pending) {
      return;
    }
    setPending(rule);
    try {
      await controller.runOperation(() =>
        setQuickRule(activeRouting, rule, enabled),
      );
    } finally {
      setPending(null);
    }
  }

  return (
    <>
      <PageSurface className="flex flex-wrap items-end gap-x-6 gap-y-3 px-4 py-3">
        <div className="grid min-w-0 gap-2">
          <h2 className="text-sm font-semibold">
            {t("panes.routing.quickRules")}
          </h2>
          <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
            {QUICK_RULES.map(({ hintKey, labelKey, rule }) => (
              <Label
                className="flex cursor-pointer items-center gap-2 text-sm font-normal"
                key={rule}
                title={t(hintKey)}
              >
                <Switch
                  aria-label={t(labelKey)}
                  checked={isQuickRuleEnabled(activeRouting, rule)}
                  disabled={!activeRouting || pending !== null}
                  onCheckedChange={(checked) => void toggle(rule, checked)}
                />
                {t(labelKey)}
              </Label>
            ))}
          </div>
          {activeRouting ? null : (
            <p className="text-xs text-muted-foreground">
              {t("panes.routing.noActiveRouting")}
            </p>
          )}
        </div>
        <SelectField
          className="ms-auto w-full sm:w-72"
          disabled={!appSettings.settings}
          label={t("panes.routing.resolveStrategy")}
          onChange={(domainStrategy) =>
            appSettings.update((current) => ({
              ...current,
              routing: { ...current.routing, domainStrategy },
            }))
          }
          options={DOMAIN_STRATEGIES.map((value) => ({
            label: t(STRATEGY_LABELS[value]),
            value,
          }))}
          value={appSettings.settings?.routing.domainStrategy ?? "AsIs"}
        />
      </PageSurface>
      <SettingsApplyStatus
        failed={Boolean(appSettings.error)}
        saving={appSettings.saving}
      />
    </>
  );
}
