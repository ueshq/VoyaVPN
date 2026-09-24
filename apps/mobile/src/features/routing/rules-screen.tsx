import { ErrorNotice } from "~/components/error-notice";
import { outboundLabelKey } from "@voya/features/routing/rule-outbound";
import { openPage } from "~/app/navigation";
import { useTrafficMode } from "@voya/features/routing/use-traffic-mode";
import { useRoutingScreen } from "@voya/features/routing/use-routing-screen";
import { ruleDisplayName } from "@voya/features/routing/sentinel-rules";
import { useI18n } from "@voya/i18n/use-i18n";
import type { TranslationKey } from "@voya/i18n/core";
import type { RoutingRule, TrafficMode } from "@voya/contracts";
import { Card } from "heroui-native/card";
import { Switch } from "heroui-native/switch";
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { Route } from "lucide-react-native";
import { useCallback, useMemo } from "react";
import { FlatList, View, useWindowDimensions } from "react-native";

import { Banner } from "~/components/banner";
import { EmptyState } from "~/components/empty-state";
import { withListPositions } from "~/components/list-positions";
import { ListRow } from "~/components/list-row";
import { PageHeader } from "~/components/page-header";
import { SectionHeader } from "~/components/section-header";
import { SegmentedControl } from "~/components/segmented-control";
import { useScreenInsets } from "~/components/use-screen-insets";

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
  const insets = useScreenInsets();
  const { fontScale, width } = useWindowDimensions();
  const routing = useRoutingScreen();
  const trafficMode = useTrafficMode();
  const rulesApply = trafficMode.mode !== "global";
  const rules = useMemo(() => withListPositions(routing.rules), [routing.rules]);

  const renderRule = useCallback(
    ({ item: { first, item, last } }: { item: { first: boolean; item: RoutingRule; last: boolean } }) => {
      const locked = !rulesApply || routing.pendingToggles.has(item.id);
      // The rules a fresh install seeds carry reserved remarks — `voya:ai-services`
      // and the like — which are an identity, not a name. The shared helper is
      // what turns them into the words the desktop shows.
      const name = ruleDisplayName(item, t);
      const outboundKey = outboundLabelKey(item.outbound ?? "proxy");
      const target = outboundKey ? t(outboundKey) : item.outbound;
      return (
        <ListRow
          inset
          first={first}
          last={last}
          dimmed={!rulesApply}
          title={name}
          description={t("mobile.ruleEffect", { target })}
          descriptionLines={0}
          onPress={() => openPage("ruleDetails", { rule: item })}
          trailingInteractive
          trailing={
            <Switch
              isSelected={item.enabled}
              isDisabled={locked}
              onSelectedChange={(enabled) => void routing.toggleRule(item, enabled)}
              accessibilityLabel={name}
              hitSlop={10}
            />
          }
        />
      );
    },
    [routing, rulesApply, t],
  );

  return (
    <View className="flex-1 bg-canvas">
      <FlatList
        data={rules}
        keyExtractor={({ item }) => item.id}
        renderItem={renderRule}
        contentContainerStyle={insets}
        // The rows read the mode and the pending set, neither of which is in
        // `data`; without this the list keeps the cells it already drew.
        extraData={renderRule}
        ListHeaderComponent={
          <View className="gap-4 px-page pb-4">
            <PageHeader title={t("tabs.rules")} />
            <View>
              <SectionHeader title={t("panes.routing.trafficMode")} />
              <Card className="gap-3 p-4">
                <SegmentedControl
                  options={TRAFFIC_MODES.map(({ labelKey, value }) => ({ label: t(labelKey), value }))}
                  value={trafficMode.mode}
                  onChange={(value) => trafficMode.selectMode(value)}
                  isDisabled={trafficMode.disabled}
                  stacked={width / fontScale < 320}
                />
                {trafficMode.disabledReason ? (
                  <Typography className="text-sm text-subtle">{t(trafficMode.disabledReason)}</Typography>
                ) : null}
              </Card>
            </View>
            {rulesApply ? null : <Banner status="warning" message={t("panes.routing.globalModeBanner")} />}
            <ErrorNotice error={routing.loadError} />
            <ErrorNotice error={routing.operationError} />
          </View>
        }
        ListFooterComponent={
          rules.length > 0 ? (
            <Typography className="px-page pt-3 text-sm text-subtle">
              {t("panes.routing.ruleOrderHint")}
            </Typography>
          ) : undefined
        }
        ListEmptyComponent={
          <View className="px-page">
            {routing.loading ? <Spinner /> : routing.loadError ? null : <EmptyState icons={[Route]} title={t("panes.routing.emptyRules")} />}
          </View>
        }
      />
    </View>
  );
}
