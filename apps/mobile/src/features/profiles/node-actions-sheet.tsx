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
import { BottomSheet } from "heroui-native/bottom-sheet";
import { Button } from "heroui-native/button";
import { Typography } from "heroui-native/text";
import { AccessibilityInfo, Alert, Share, findNodeHandle, StyleSheet, View, useWindowDimensions, type Text } from "react-native";
import { BottomSheetScrollView, type BottomSheetBackgroundProps } from "@gorhom/bottom-sheet";
import { QrCode, Copy, Gauge, Share2, Trash2, type LucideIcon } from "lucide-react-native";
import { useRef, useState, type ReactNode } from "react";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { SvgXml } from "react-native-svg";

import { ListCard } from "~/components/list-card";
import { ListRow } from "~/components/list-row";
import { useToneColor } from "~/components/tone";

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
 *
 * The sheet is a HeroUI `BottomSheet` (gorhom under the hood) rather than a
 * hand-rolled `Modal`: dynamic height, drag-to-dismiss, and the portal host
 * that keeps the panel above the tab bar. Portal content renders at the host,
 * not where it is declared — so this file takes already-translated strings as
 * props and calls no context hook inside the sheet body.
 *
 * The default gorhom background and handle announce themselves to VoiceOver
 * as "BottomSheet" / "Bottom sheet handle". Both are replaced with unlabelled
 * views (`accessible={false}`) so a screen reader only lands on the actions;
 * the explicit Close button is the accessible way out.
 */
export function NodeActionsSheet({
  entry: activeEntry,
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
  // Preserve the measured content while the sheet closes. Removing the rows
  // during the close animation can leave the bounded scroll sheet without a
  // usable layout when the same node is opened again on a small screen.
  const [lastEntry, setLastEntry] = useState(activeEntry);
  if (activeEntry !== null && activeEntry !== lastEntry) setLastEntry(activeEntry);
  const entry = activeEntry ?? lastEntry;
  const queryClient = useQueryClient();
  const share = exports.shareQrContent;
  const insets = useSafeAreaInsets();
  const { height, fontScale } = useWindowDimensions();
  const scrollable = fontScale > 1.2 || height < 700;
  const titleRef = useRef<Text>(null);
  // Resolved here, where the sheet is declared: the body renders at the
  // portal host, so it takes finished values rather than reading context.
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
    // HeroUI's Content.onClose only covers swipe dismissal. Restore after the
    // parent has rendered the background accessible again for every exit path.
    requestAnimationFrame(onClosed);
  }

  async function remove(indexId: string) {
    const removed = await operation.runOperation(async () => {
      await deleteSafely([indexId], () => voyaCommands().deleteProfiles([indexId]));
      await queryClient.invalidateQueries();
    });
    if (removed) close();
  }

  const open = activeEntry !== null || share !== null;

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
    <BottomSheet
      isOpen={open}
      onOpenChange={(next) => {
        if (!next) close();
      }}
    >
      <BottomSheet.Portal
        // A plain View is what XCUITest can see into; FullWindowOverlay puts
        // the panel in another window the test runner does not walk.
        disableFullWindowOverlay
      >
        <View
          style={StyleSheet.absoluteFill}
          accessible={false}
          accessibilityViewIsModal={open}
          accessibilityElementsHidden={!open}
          // Gorhom keeps the closed sheet mounted below its container. Its
          // dynamic resize can leave a hit region over the tabs during close.
          // Gate the whole portal, including gestures, as soon as it closes.
          pointerEvents={open ? "box-none" : "none"}
        >
          <BottomSheet.Overlay accessible={false} />
          <BottomSheet.Content
            // Ordinary sheets size to content; large text gets a bounded
            // scroll view so the close action stays reachable.
            accessible={false}
            accessibilityViewIsModal={open}
            accessibilityElementsHidden={!open}
            contentContainerProps={{ accessible: false, accessibilityViewIsModal: open, accessibilityElementsHidden: !open }}
            enableDynamicSizing={!scrollable}
            enableOverDrag={false}
            snapPoints={scrollable ? ["85%"] : undefined}
            contentContainerClassName={scrollable ? "h-full" : undefined}
            maxDynamicContentSize={height - insets.top - 16}
            onChange={(index) => {
              if (index >= 0) focusTitle();
            }}
            // The canvas, not a card surface: the actions inside are white
            // cards of their own, the grouped look the rest of the app uses.
            // It goes through HeroUI's class because gorhom hands the
            // background a style that would override a class set on it.
            backgroundClassName="bg-canvas"
            backgroundComponent={SheetBackground}
            handleComponent={SheetHandle}
          >
            <SheetBody scrollable={scrollable} bottomInset={insets.bottom}>
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
            </SheetBody>
          </BottomSheet.Content>
        </View>
      </BottomSheet.Portal>
    </BottomSheet>
  );
}

/** Unlabelled surface so VoiceOver does not announce a bare "BottomSheet". */
function SheetBackground({ style, pointerEvents }: BottomSheetBackgroundProps) {
  return <View accessible={false} pointerEvents={pointerEvents} style={style} />;
}

function SheetBody({ children, scrollable, bottomInset }: { children: ReactNode; scrollable: boolean; bottomInset: number }) {
  const paddingBottom = Math.max(16, bottomInset);
  return scrollable ? (
    <BottomSheetScrollView contentContainerStyle={{ padding: 16, paddingBottom, gap: 16 }}>
      {children}
    </BottomSheetScrollView>
  ) : <View style={{ paddingBottom }} className="gap-4 p-page">{children}</View>;
}

/** Unlabelled grabber; the drag itself is the affordance. */
function SheetHandle() {
  return (
    <View accessible={false} className="items-center py-2">
      <View className="h-1 w-10 rounded-full bg-subtlest" />
    </View>
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
