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
import { AccessibilityInfo, findNodeHandle, StyleSheet, View, useWindowDimensions, type Text } from "react-native";
import { BottomSheetScrollView, type BottomSheetBackgroundProps } from "@gorhom/bottom-sheet";
import { useRef, type ReactNode } from "react";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { SvgXml } from "react-native-svg";

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
  entry,
  exports,
  onClose,
  onClosed,
  operation,
}: {
  entry: ProfileSummaryEntry | null;
  exports: ReturnType<typeof useNodeExport>;
  onClose: () => void;
  onClosed: () => void;
  operation: NodeOperation;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const share = exports.shareQrContent;
  const insets = useSafeAreaInsets();
  const { height, fontScale } = useWindowDimensions();
  const scrollable = fontScale > 1.2 || height < 700;
  const titleRef = useRef<Text>(null);

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
      await voyaCommands().deleteProfiles([indexId]);
      await queryClient.invalidateQueries();
    });
    if (removed) close();
  }

  const open = entry !== null || share !== null;

  const shareLabel = t("panes.profiles.export.shareLinks");
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
            backgroundComponent={SheetBackground}
            handleComponent={SheetHandle}
          >
            <SheetBody scrollable={scrollable} bottomInset={insets.bottom}>
              {share ? (
                <View className="items-center gap-3">
                  <Typography ref={titleRef} onLayout={focusTitle} accessibilityRole="header" className="text-section text-foreground">{qrTitle}</Typography>
                  {qrQuery.data ? (
                    <View
                      // `accessible` is what turns the label into something a
                      // screen reader can land on: a bare View carrying an
                      // `accessibilityLabel` is not an accessibility element in
                      // React Native, so the label would be dropped on the floor.
                      accessible
                      accessibilityRole="image"
                      className="aspect-square w-full max-w-72 bg-white p-3"
                      accessibilityLabel={qrAlt}
                    >
                      <SvgXml xml={qrQuery.data.svg} width="100%" height="100%" />
                    </View>
                  ) : (
                    <Typography className="text-caption text-subtle">
                      {qrQuery.error ? failedLabel : loadingLabel}
                    </Typography>
                  )}
                </View>
              ) : entry ? (
                <>
                  <Typography ref={titleRef} onLayout={focusTitle} accessibilityRole="header" className="text-section text-foreground" numberOfLines={2}>
                    {title}
                  </Typography>
                  <SheetAction
                    label={shareLabel}
                    onPress={() => {
                      void exports.handleExport([entry.profile.id]);
                      close();
                    }}
                  />
                  <SheetAction
                    label={showQrLabel}
                    onPress={() => void exports.handleExport([entry.profile.id], "qr")}
                  />
                  {entry.profile.subscriptionId === null ? (
                    <SheetAction
                      destructive
                      label={deleteLabel}
                      onPress={() => void remove(entry.profile.id)}
                    />
                  ) : (
                    <Typography className="text-caption text-subtle">
                      {subscriptionReadOnly}
                    </Typography>
                  )}
                </>
              ) : null}
              <SheetAction label={closeLabel} onPress={close} />
            </SheetBody>
          </BottomSheet.Content>
        </View>
      </BottomSheet.Portal>
    </BottomSheet>
  );
}

/** Unlabelled surface so VoiceOver does not announce a bare "BottomSheet". */
function SheetBackground({ style, pointerEvents }: BottomSheetBackgroundProps) {
  return <View accessible={false} pointerEvents={pointerEvents} style={style} className="rounded-t-3xl bg-surface" />;
}

function SheetBody({ children, scrollable, bottomInset }: { children: ReactNode; scrollable: boolean; bottomInset: number }) {
  const paddingBottom = Math.max(16, bottomInset);
  return scrollable ? (
    <BottomSheetScrollView contentContainerStyle={{ padding: 16, paddingBottom, gap: 12 }}>
      {children}
    </BottomSheetScrollView>
  ) : <View style={{ paddingBottom }} className="gap-3 p-page">{children}</View>;
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
 * label is already a string by the time it gets here.
 */
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
    <Button
      className="min-h-11 h-auto py-3"
      variant={destructive ? "danger-soft" : "tertiary"}
      onPress={onPress}
      accessibilityRole="button"
    >
      <Button.Label>{label}</Button.Label>
    </Button>
  );
}
