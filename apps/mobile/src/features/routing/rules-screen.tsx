import { useTrafficMode } from "@voya/features/routing/use-traffic-mode";
import { useRoutingScreen } from "@voya/features/routing/use-routing-screen";
import { ruleMatchChips, type MatchChip } from "@voya/features/routing/rule-match-summary";
import { ruleDisplayName } from "@voya/features/routing/sentinel-rules";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n/core";
import type { RoutingRule, TrafficMode } from "@voya/contracts";
import { useCallback } from "react";
import { FlatList, Switch, View } from "react-native";

import { Pressable } from "~/components/ui/pressable";
import { Text } from "~/components/ui/text";

/**
 * What a rule matches, in one line.
 *
 * The chips are the shared shaping; a phone row has no space for the desktop's
 * separate badges, so they read as text. Nothing here is translated because
 * nothing here is a word: they are the user's own domains, addresses and ports.
 */
function matchSummary(rule: RoutingRule) {
  return ruleMatchChips(rule).map(chipText).join(" · ");
}

function chipText(chip: MatchChip) {
  switch (chip.kind) {
    case "list":
      return chip.more > 0 ? `${chip.first} +${chip.more}` : chip.first;
    case "port":
      return [chip.network, chip.port].filter(Boolean).join(":");
    case "scope":
      return chip.scope;
  }
}

/**
 * The two modes a user can pick, in order.
 *
 * `TrafficMode` also has `unchanged`, which is what the backend reports when
 * it has not been told yet; it is a state, not a choice, so it is not offered.
 */
const TRAFFIC_MODES = [
  { labelKey: "panes.routing.trafficModeRule", value: "rule" },
  { labelKey: "proxy.trafficModeGlobal", value: "global" },
] as const satisfies readonly { labelKey: TranslationKey; value: TrafficMode }[];

/**
 * The Rules screen.
 *
 * It edits the active rule set only — the one the core runs with — and the
 * traffic mode sits above it, because in global mode every rule is bypassed
 * and the list greys out to say so. Both come from the same controllers the
 * desktop Rules page uses.
 */
export function RulesScreen() {
  const { t } = useI18n();
  const routing = useRoutingScreen();
  const trafficMode = useTrafficMode();
  const rulesApply = trafficMode.mode !== "global";

  const renderRule = useCallback(
    ({ item }: { item: RoutingRule }) => {
      const locked = !rulesApply || routing.pendingToggles.has(item.id);
      // The rules a fresh install seeds carry reserved remarks — `voya:ai-services`
      // and the like — which are an identity, not a name. The shared helper is
      // what turns them into the words the desktop shows.
      const name = ruleDisplayName(item, t);
      return (
        <View
          className={`flex-row items-center justify-between border-b border-border-subtle bg-surface px-4 py-3 ${
            rulesApply ? "" : "opacity-40"
          }`}
        >
          <View className="flex-1 gap-0.5 pr-3">
            <Text className="text-body text-foreground" numberOfLines={1}>
              {name}
            </Text>
            <Text className="text-caption text-subtlest" numberOfLines={1}>
              {matchSummary(item)}
            </Text>
          </View>
          <Switch
            value={item.enabled}
            disabled={locked}
            onValueChange={(enabled) => void routing.toggleRule(item, enabled)}
            accessibilityLabel={name}
            // iOS's switch carries `disabled` natively but says nothing about
            // it, so the state is spelled out for VoiceOver either way.
            accessibilityState={{ disabled: locked }}
          />
        </View>
      );
    },
    [routing, rulesApply, t],
  );

  return (
    <View className="flex-1 bg-canvas">
      <View className="gap-2 p-page">
        <Text className="text-caption uppercase text-subtle">{t("panes.routing.trafficMode")}</Text>
        <View className="flex-row gap-2">
          {TRAFFIC_MODES.map(({ labelKey, value }) => (
            <Pressable
              key={value}
              className={`flex-1 items-center rounded-control border px-3 py-2 ${
                trafficMode.mode === value
                  ? "border-brand bg-brand-tint"
                  : "border-border bg-surface"
              }`}
              disabled={trafficMode.disabled}
              onPress={() => trafficMode.selectMode(value)}
              accessibilityRole="button"
              accessibilityState={{ selected: trafficMode.mode === value }}
            >
              <Text className="text-body text-foreground">{t(labelKey)}</Text>
            </Pressable>
          ))}
        </View>
        {trafficMode.disabledReason ? (
          <Text className="text-caption text-subtle">{t(trafficMode.disabledReason)}</Text>
        ) : null}
        {rulesApply ? null : (
          <Text className="text-caption text-warning">{t("panes.routing.globalModeBanner")}</Text>
        )}
        {routing.loadError ? (
          <Text className="text-caption text-danger">{routing.loadError}</Text>
        ) : null}
        {routing.operationError ? (
          <Text className="text-caption text-danger">{routing.operationError}</Text>
        ) : null}
      </View>

      <FlatList
        data={[...routing.rules]}
        keyExtractor={(rule) => rule.id}
        renderItem={renderRule}
        // The rows read the mode and the pending set, neither of which is in
        // `data`; without this the list keeps the cells it already drew.
        extraData={renderRule}
        ListEmptyComponent={
          <View className="items-center p-page">
            <Text className="text-body text-subtle">
              {t("panes.routing.emptyRules")}
            </Text>
          </View>
        }
      />
    </View>
  );
}
