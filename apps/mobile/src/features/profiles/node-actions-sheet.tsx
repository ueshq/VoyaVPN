import { deleteSafely } from "./delete-safely";
import { openPage } from "~/app/navigation";
import { voyaCommands } from "@voya/client/transport";
import { profileTitle } from "@voya/features/profiles/profile-display";
import type { NodeOperation } from "@voya/features/profiles/use-node-operation";
import type { useNodeExport } from "@voya/features/profiles/use-node-export";
import { useI18n } from "@voya/i18n/use-i18n";
import { useQueryClient } from "@tanstack/react-query";
import type { ProfileSummaryEntry } from "@voya/contracts";
import { useQuery } from "@tanstack/react-query";
import { Button } from "heroui-native/button";
import { Typography } from "heroui-native/text";
import { AccessibilityInfo, Alert, Modal, Pressable, ScrollView, Share, findNodeHandle, StyleSheet, View, useWindowDimensions, type Text } from "react-native";
import { QrCode, Copy, Gauge, Share2, Trash2, type LucideIcon } from "lucide-react-native";
import { useRef } from "react";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { SvgXml } from "react-native-svg";

import { ListCard } from "~/components/list-card";
import { ListRow } from "~/components/list-row";
import { useToneColor } from "~/components/tone";

/** Node actions use a native modal boundary so opening during a long press
 * cannot race a closing bottom-sheet gesture. HeroUI rows/buttons retain the
 * app's grouped styling; bounded scrolling keeps every action reachable. */
export function NodeActionsSheet({
  entry,
  exports,
  onClose,
  onClosed,
  operation,
  onTest,
}: {
  entry: ProfileSummaryEntry | null;
  exports: ReturnType<typeof useNodeExport>;
  onClose: () => void;
  onClosed: () => void;
  operation: NodeOperation;
  onTest: (id: string) => void;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const share = exports.shareQrContent;
  const insets = useSafeAreaInsets();
  const { height } = useWindowDimensions();
  const titleRef = useRef<Text>(null);
  // Resolve semantic colors once for every action in this modal.
  const actionColor = useToneColor("brand");
  const dangerColor = useToneColor("danger");

  // Rendered by the backend so both shells show the same code; a phone has no
  // canvas to draw one on anyway.
  const qrQuery = useQuery({
    enabled: share !== null,
    queryFn: () => voyaCommands().generateQrCode(share ?? ""),
    queryKey: ["mobile", "share-qr", share],
  });

  function focusTitle() {
    const target = findNodeHandle(titleRef.current);
    if (target != null) AccessibilityInfo.setAccessibilityFocus(target);
  }

  function close() {
    exports.setShareQrContent(null);
    onClose();
    // Restore after React has made the background accessible again. iOS also
    // restores onDismiss after the native presentation has fully disappeared.
    requestAnimationFrame(onClosed);
  }

  async function remove(indexId: string) {
    const removed = await operation.runOperation(async () => {
      await deleteSafely([indexId], () => voyaCommands().deleteProfiles([indexId]));
      await queryClient.invalidateQueries();
    });
    if (removed) close();
  }

  const open = entry !== null || share !== null;

  const shareLabel = t("mobile.copy");
  const showQrLabel = t("panes.profiles.export.showQr");
  const deleteLabel = t("actions.delete");
  const closeLabel = t("actions.close");
  const title = entry ? profileTitle(entry.profile.remarks, t) : "";
  const qrTitle = t("panes.profiles.export.showQr");
  const qrAlt = t("qr.generatedAlt");
  const loadingLabel = t("panes.profiles.loadingNodes");
  const failedLabel = t("status.operationFailed");
  const subscriptionReadOnly = t("panes.profiles.menu.subscriptionReadOnly");

  return (
    <Modal visible={open} transparent animationType="fade" onRequestClose={close} onShow={focusTitle} onDismiss={onClosed}>
      <View style={{ flex: 1, justifyContent: "flex-end", paddingTop: insets.top + 16 }}>
        <Pressable accessible={false} style={[StyleSheet.absoluteFill, { backgroundColor: "rgba(0,0,0,0.3)" }]} onPress={close} />
        <View accessibilityViewIsModal className="rounded-t-3xl bg-canvas" style={{ maxHeight: height - insets.top - 16 }}>
          <ScrollView key={share ? "qr" : "actions"} style={{ flexGrow: 0 }} contentContainerStyle={{ padding: 20, paddingBottom: Math.max(20, insets.bottom), gap: 16 }}>
              {share ? (
                <View className="items-center gap-4">
                  <Typography ref={titleRef} onLayout={focusTitle} accessibilityRole="header" maxFontSizeMultiplier={2} className="text-xl font-semibold text-foreground">{qrTitle}</Typography>
                  <Typography className="text-base text-foreground">{title}</Typography>
                  <Typography className="text-sm text-subtle">{entry?.profile.kind} · {entry?.profile.address}</Typography>
                  {qrQuery.data ? (
                    <View
                      // `accessible` is what turns the label into something a
                      // screen reader can land on: a bare View carrying an
                      // `accessibilityLabel` is not an accessibility element in
                      // React Native, so the label would be dropped on the floor.
                      accessible
                      accessibilityRole="image"
                      className="aspect-square w-full max-w-72 rounded-3xl bg-white p-4 shadow-surface"
                      accessibilityLabel={qrAlt}
                    >
                      <SvgXml xml={qrQuery.data.svg} width="100%" height="100%" />
                    </View>
                  ) : (
                    <Typography className="text-sm text-subtle">
                      {qrQuery.error ? failedLabel : loadingLabel}
                    </Typography>
                  )}
                </View>
              ) : entry ? (
                <>
                  <View className="gap-1 px-1">
                    <Typography ref={titleRef} onLayout={focusTitle} accessibilityRole="header" maxFontSizeMultiplier={2} className="text-xl font-semibold text-foreground" numberOfLines={2}>
                      {title}
                    </Typography>
                    <Typography className="text-sm text-subtle" numberOfLines={1}>{entry.profile.address}</Typography>
                  </View>
                  <ListCard>
                    <SheetAction
                      icon={Copy}
                      color={actionColor}
                      label={shareLabel}
                      onPress={() => {
                        void exports.handleExport([entry.profile.id]);
                        close();
                      }}
                    />
                    <SheetAction icon={Share2} color={actionColor} label={t("mobile.share")} onPress={() => {
                      void operation.runOperation(async () => {
                        const result = await voyaCommands().exportProfileShareLinks([entry.profile.id]);
                        await Share.share({ message: result.text });
                      });
                    }} />
                    <SheetAction icon={Gauge} color={actionColor} label={t("mobile.testNode")} onPress={() => { onTest(entry.profile.id); close(); }} />
                    <SheetAction
                      icon={QrCode}
                      color={actionColor}
                      label={showQrLabel}
                      last={entry.profile.subscriptionId !== null}
                      onPress={() => void exports.handleExport([entry.profile.id], "qr")}
                    />
                    {entry.profile.subscriptionId === null ? (
                      <SheetAction
                        destructive
                        icon={Trash2}
                        color={dangerColor}
                        label={deleteLabel}
                        last
                        onPress={() => Alert.alert(t("mobile.deleteConfirm"), t("mobile.deleteHint"), [
                          { text: t("actions.cancel"), style: "cancel" },
                          { text: deleteLabel, style: "destructive", onPress: () => void remove(entry.profile.id) },
                        ])}
                      />
                    ) : null}
                  </ListCard>
                  {entry.profile.subscriptionId === null ? null : (
                    <View className="gap-2">
                      <Typography className="px-1 text-sm text-subtle">{subscriptionReadOnly}</Typography>
                      <Button className="min-h-12 h-auto" variant="secondary" onPress={() => { const id = entry.profile.subscriptionId; close(); if (id) openPage("subscription", { id }); }}><Button.Label>{t("mobile.subscriptions")}</Button.Label></Button>
                    </View>
                  )}
                </>
              ) : null}
              <Button
                className="min-h-12 h-auto rounded-3xl py-3"
                variant="tertiary"
                onPress={close}
                accessibilityRole="button"
              >
                <Button.Label>{closeLabel}</Button.Label>
              </Button>
          </ScrollView>
        </View>
      </View>
    </Modal>
  );
}

/**
 * One sheet row. Pure props — the portal body cannot read context, so the
 * label and the icon colour are already values by the time they get here.
 */
function SheetAction({
  color,
  destructive = false,
  icon: Icon,
  label,
  last = false,
  onPress,
}: {
  color: string | undefined;
  destructive?: boolean;
  icon: LucideIcon;
  label: string;
  last?: boolean;
  onPress: () => void;
}) {
  return (
    <ListRow
      last={last}
      title={label}
      titleClassName={destructive ? "text-danger" : "text-foreground"}
      leading={<Icon size={20} color={color} accessible={false} />}
      onPress={onPress}
      accessibilityLabel={label}
    />
  );
}
