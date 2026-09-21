import { voyaCommands } from "@voya/client/transport";
import { profileTitle } from "@voya/features/profiles/profile-display";
import type { NodeOperation } from "@voya/features/profiles/use-node-operation";
import type { useNodeExport } from "@voya/features/profiles/use-node-export";
import { useI18n } from "@voya/i18n/use-i18n";
import { useQueryClient } from "@tanstack/react-query";
import type { ProfileSummaryEntry } from "@voya/contracts";
import { useQuery } from "@tanstack/react-query";
import { Modal, Pressable as RNPressable, View } from "react-native";
import { SvgXml } from "react-native-svg";

import { Text } from "~/components/ui/text";

/**
 * What a node row can do beyond being selected.
 *
 * A phone has no right-click and no hover, so the desktop's row menu becomes a
 * long press and this sheet. It carries the two actions that are useful away
 * from a keyboard — hand the link to another app, or show it as a QR for a
 * device that cannot be pasted into — and deleting, which is the one
 * destructive thing a phone should still be able to do.
 *
 * Editing is not here. The desktop dialog is a protocol, transport and TLS
 * form; retyping one of those on a phone is not a feature, and every ordinary
 * way a node arrives — a share link, a subscription, a QR — already works.
 */
export function NodeActionsSheet({
  entry,
  exports,
  onClose,
  operation,
}: {
  entry: ProfileSummaryEntry | null;
  exports: ReturnType<typeof useNodeExport>;
  onClose: () => void;
  operation: NodeOperation;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const share = exports.shareQrContent;

  // Rendered by the backend so both shells show the same code; a phone has no
  // canvas to draw one on anyway.
  const qrQuery = useQuery({
    enabled: share !== null,
    queryFn: () => voyaCommands().generateQrCode(share ?? ""),
    queryKey: ["mobile", "share-qr", share],
  });

  function close() {
    exports.setShareQrContent(null);
    onClose();
  }

  async function remove(indexId: string) {
    await operation.runOperation(async () => {
      await voyaCommands().deleteProfiles([indexId]);
      await queryClient.invalidateQueries();
    });
    close();
  }

  const open = entry !== null || share !== null;

  return (
    <Modal
      visible={open}
      transparent
      animationType="slide"
      onRequestClose={close}
      accessibilityViewIsModal
    >
      {/* Tapping away closes it, the way every sheet on a phone does.
          Both wrappers are `accessible={false}`: React Native's `Pressable`
          makes itself an accessibility element by default, which collapsed the
          whole sheet into one node announced as
          "Simulator node, Share links, Show QR, Delete, Close" — a single blob
          with no way to activate any one action, for VoiceOver or for anything
          else. The scrim loses nothing by it, because the explicit Close button
          below is the accessible way out, and the inner wrapper only exists to
          stop the tap from reaching the scrim. */}
      <RNPressable
        accessible={false}
        className="flex-1 justify-end bg-black/40"
        onPress={close}
      >
        <RNPressable
          accessible={false}
          className="gap-3 rounded-t-card bg-card p-page"
          onPress={() => {}}
        >
          {share ? (
            <View className="items-center gap-3">
              <Text className="text-section text-foreground">
                {t("panes.profiles.export.showQr")}
              </Text>
              {qrQuery.data ? (
                <View
                  // `accessible` is what turns the label into something a
                  // screen reader can land on: a bare View carrying an
                  // `accessibilityLabel` is not an accessibility element in
                  // React Native, so the label was dropped on the floor.
                  accessible
                  accessibilityRole="image"
                  className="aspect-square w-full max-w-72 bg-white p-3"
                  accessibilityLabel={t("qr.generatedAlt")}
                >
                  <SvgXml xml={qrQuery.data.svg} width="100%" height="100%" />
                </View>
              ) : (
                <Text className="text-caption text-subtle">
                  {qrQuery.error
                    ? t("status.operationFailed")
                    : t("panes.profiles.loadingNodes")}
                </Text>
              )}
            </View>
          ) : entry ? (
            <>
              <Text className="text-section text-foreground" numberOfLines={1}>
                {profileTitle(entry.profile.remarks, t)}
              </Text>
              <SheetAction
                label={t("panes.profiles.export.shareLinks")}
                onPress={() => {
                  void exports.handleExport([entry.profile.id]);
                  close();
                }}
              />
              <SheetAction
                label={t("panes.profiles.export.showQr")}
                onPress={() => void exports.handleExport([entry.profile.id], "qr")}
              />
              {entry.profile.subscriptionId === null ? (
                <SheetAction
                  destructive
                  label={t("actions.delete")}
                  onPress={() => void remove(entry.profile.id)}
                />
              ) : (
                <Text className="text-caption text-subtle">
                  {t("panes.profiles.menu.subscriptionReadOnly")}
                </Text>
              )}
            </>
          ) : null}
          <SheetAction label={t("actions.close")} onPress={close} />
        </RNPressable>
      </RNPressable>
    </Modal>
  );
}

function SheetAction({
  destructive = false,
  label,
  onPress,
}: {
  destructive?: boolean;
  label: string;
  onPress: () => void;
}) {
  return (
    <RNPressable
      className="rounded-control bg-surface px-4 py-3"
      onPress={onPress}
      accessibilityRole="button"
    >
      <Text className={`text-body ${destructive ? "text-danger" : "text-foreground"}`}>
        {label}
      </Text>
    </RNPressable>
  );
}
