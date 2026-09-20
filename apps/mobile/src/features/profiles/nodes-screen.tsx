import { useProfileActivation } from "@voya/client/runtime-action";
import type { NodeListRow } from "@voya/features/profiles/node-list-rows";
import { profileLatency, profileTitle } from "@voya/features/profiles/profile-display";
import { useNodeImport } from "@voya/features/profiles/use-node-import";
import { useNodeListData } from "@voya/features/profiles/use-node-list-data";
import { useNodeOperation } from "@voya/features/profiles/use-node-operation";
import { useI18n } from "@voya/i18n/use-i18n";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";
import { FlatList, View } from "react-native";

import { Button, ButtonSpinner, ButtonText } from "~/components/ui/button";
import { Input, InputField } from "~/components/ui/input";
import { Pressable } from "~/components/ui/pressable";
import { Text } from "~/components/ui/text";

import { useNodeSelection } from "./use-node-selection";

/**
 * The node list.
 *
 * The rows come from `nodeListRows` through `useNodeListData` — the same
 * shaping the desktop table uses, group headers included — so a list that is
 * grouped, filtered and sorted one way there is grouped, filtered and sorted
 * the same way here. A `FlatList` renders them directly rather than a
 * `SectionList` re-deriving sections the shaping already decided.
 */
export function NodesScreen() {
  const { t } = useI18n();
  const selection = useNodeSelection();
  const data = useNodeListData(selection, t);
  const operation = useNodeOperation();
  const activation = useProfileActivation(t);
  const queryClient = useQueryClient();

  const onImported = useCallback(async () => {
    await queryClient.invalidateQueries();
  }, [queryClient]);
  const imports = useNodeImport(operation, onImported, t);

  const renderRow = useCallback(
    ({ item }: { item: NodeListRow }) =>
      item.kind === "group" ? (
        <Pressable
          className="flex-row items-center justify-between bg-canvas px-4 py-2"
          onPress={() => selection.toggleGroup(item.groupKey)}
          accessibilityRole="button"
        >
          <Text className="text-caption font-medium uppercase text-subtle">{item.name}</Text>
          <Text className="text-caption text-subtlest">
            {t("nodeGroups.membersCount", { count: item.allMembers.length })}
          </Text>
        </Pressable>
      ) : (
        <Pressable
          className="flex-row items-center justify-between border-b border-border-subtle bg-surface px-4 py-3"
          disabled={activation.busy}
          onPress={() => void activation.activateProfile(item.item.profile.id)}
          accessibilityRole="button"
        >
          <View className="flex-1 gap-0.5 pr-3">
            <Text className="text-body text-foreground" numberOfLines={1}>
              {profileTitle(item.item.profile.remarks, t)}
            </Text>
            <Text className="text-caption text-subtlest" numberOfLines={1}>
              {item.item.profile.address}
            </Text>
          </View>
          <View className="items-end gap-0.5">
            <Text className="text-caption text-subtle">{profileLatency(item.item, t)}</Text>
            {activation.runningId === item.item.profile.id ? (
              <Text className="text-caption text-connected">{t("panes.profiles.card.using")}</Text>
            ) : item.item.isActive ? (
              <Text className="text-caption text-brand">{t("panes.profiles.card.default")}</Text>
            ) : null}
          </View>
        </Pressable>
      ),
    [activation, selection, t],
  );

  return (
    <View className="flex-1 bg-canvas">
      <View className="gap-2 p-page">
        <Input>
          <InputField
            placeholder={t("panes.profiles.search.placeholder")}
            value={selection.search}
            onChangeText={selection.setSearch}
            autoCapitalize="none"
            autoCorrect={false}
          />
        </Input>
        <Button
          variant="outline"
          isDisabled={imports.directImportPending !== null}
          onPress={() => void imports.handleDirectImport("clipboard")}
        >
          {imports.directImportPending ? <ButtonSpinner /> : null}
          <ButtonText>{t("panes.profiles.import.clipboard")}</ButtonText>
        </Button>
        {operation.operationError ? (
          <Text className="text-caption text-danger">{operation.operationError}</Text>
        ) : null}
      </View>

      <FlatList
        data={data.rows}
        keyExtractor={(row) => row.key}
        renderItem={renderRow}
        ListEmptyComponent={
          <View className="items-center gap-1 p-page">
            <Text className="text-body text-foreground">
              {selection.search ? t("panes.profiles.search.empty") : t("panes.profiles.empty")}
            </Text>
            <Text className="text-caption text-subtle">
              {selection.search
                ? t("panes.profiles.search.emptyHint")
                : t("panes.profiles.emptyDescription")}
            </Text>
          </View>
        }
      />
    </View>
  );
}
