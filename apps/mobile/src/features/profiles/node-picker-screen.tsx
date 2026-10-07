import { useQuery } from "@tanstack/react-query";
import { queries } from "@voya/client/queries";
import { useProfileActivation } from "@voya/client/runtime-action";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { usePolicyGroups } from "@voya/features/profiles/use-policy-groups";
import { useNodeOperation } from "@voya/features/profiles/use-node-operation";
import { profileTitle } from "@voya/features/profiles/profile-display";
import { POLICY_GROUP_STRATEGY_HINT_KEYS } from "@voya/features/profiles/policy-group-labels";
import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "heroui-native/button";
import { SearchField } from "heroui-native/search-field";
import { Typography } from "heroui-native/text";
import { useMemo, useState } from "react";
import { FlatList, View } from "react-native";
import { navigateToTab } from "~/app/navigation";
import { useScreenInsets } from "~/components/use-screen-insets";
import { ListRow } from "~/components/list-row";
import { SelectionMark } from "~/components/selection-mark";
import { ErrorNotice } from "~/components/error-notice";
import { useConnectionPreferences } from "@voya/client/connection-preferences";

/** A stack modal: native dismissal/back behavior and no management tools in the selection path. */
export function NodePickerScreen() {
  const { t } = useI18n();
  const insets = useScreenInsets();
  const query = useQuery(queries.profileList);
  const activation = useProfileActivation(t, { inline: true });
  const operation = useNodeOperation();
  const groups = usePolicyGroups(operation, t, { live: false, inline: true });
  const connected = useRuntimeEventStore((s) => s.coreState?.state === "connected");
  const recent = useConnectionPreferences((s) => s.recentIds);
  const [search, setSearch] = useState("");
  const [error, setError] = useState<unknown>(null);
  const rows = useMemo(() => {
    const needle = search.trim().toLocaleLowerCase();
    const groupRows = groups.policyGroupEntries.map((entry) => ({
      key: `group:${entry.group.id}`,
      id: entry.group.id,
      group: true,
      name: entry.group.name,
      active: entry.isActive && (!connected || activation.runningId === null),
      description: t(POLICY_GROUP_STRATEGY_HINT_KEYS[entry.group.strategy]),
    }));
    const nodes = (query.data?.entries ?? []).map((entry) => ({
      key: entry.profile.id,
      id: entry.profile.id,
      group: false,
      name: profileTitle(entry.profile.remarks, t),
      active: activation.runningId === entry.profile.id || (!connected && entry.isActive),
      description: recent.includes(entry.profile.id) ? t("daily.recent") : undefined,
    }));
    return [...groupRows, ...nodes]
      .filter((row) => row.name.toLocaleLowerCase().includes(needle))
      .sort((a, b) => {
        if (a.active !== b.active) return a.active ? -1 : 1;
        const aRank = recent.indexOf(a.key),
          bRank = recent.indexOf(b.key);
        return (aRank < 0 ? 99 : aRank) - (bRank < 0 ? 99 : bRank);
      });
  }, [query.data, groups.policyGroupEntries, activation.runningId, connected, recent, search, t]);
  async function choose(id: string, group: boolean) {
    setError(null);
    try {
      const selected = await (group ? groups.selectPolicyGroup(id) : activation.selectProfile(id));
      if (selected) navigateToTab("home");
      else setError(t("daily.switchFailed"));
    } catch (failure) {
      setError(failure);
    }
  }
  return (
    <FlatList
      data={rows}
      keyExtractor={(item) => item.key}
      contentContainerStyle={insets}
      keyboardShouldPersistTaps="handled"
      keyboardDismissMode="on-drag"
      className="flex-1 bg-canvas"
      ListHeaderComponent={
        <View className="gap-3 px-page pb-4">
          <Typography className="text-base text-subtle">
            {t(connected ? "daily.switchHint" : "daily.selectionHint")}
          </Typography>
          <SearchField value={search} onChange={setSearch}>
            <SearchField.Group>
              <SearchField.SearchIcon />
              <SearchField.Input
                accessibilityLabel={t("daily.searchNodes")}
                placeholder={t("daily.searchNodes")}
                autoCapitalize="none"
                autoCorrect={false}
              />
              <SearchField.ClearButton accessibilityLabel={t("activity.clearSearch")} />
            </SearchField.Group>
          </SearchField>
          <ErrorNotice error={query.error} retry={() => void query.refetch()} />
          <ErrorNotice error={groups.policyGroupsError} retry={groups.retryPolicyGroups} />
          <ErrorNotice error={error} message={t("daily.switchFailed")} />
        </View>
      }
      renderItem={({ item, index }) => (
        <ListRow
          inset
          first={index === 0}
          last={index === rows.length - 1}
          title={item.name}
          description={item.description}
          titleLines={2}
          leading={<SelectionMark state={item.active ? (connected ? "inUse" : "selected") : "none"} />}
          isDisabled={activation.busy}
          accessibilityState={{ selected: item.active, busy: activation.busy }}
          onPress={() => {
            if (item.active) navigateToTab("home");
            else void choose(item.id, item.group);
          }}
        />
      )}
      ListEmptyComponent={
        query.isError ? undefined : (
          <Typography className="px-page text-base text-subtle">
            {t(query.isPending ? "panes.profiles.loadingNodes" : "panes.profiles.search.empty")}
          </Typography>
        )
      }
      ListFooterComponent={
        <View className="gap-3 p-page">
          <Button variant="secondary" onPress={() => navigateToTab("profiles")}>
            <Button.Label>{t("daily.manageNodes")}</Button.Label>
          </Button>
        </View>
      }
    />
  );
}
