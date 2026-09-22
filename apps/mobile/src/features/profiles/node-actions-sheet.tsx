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
import { View } from "react-native";
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
        <BottomSheet.Overlay accessible={false} />
        <BottomSheet.Content
          // No `snapPoints`: gorhom v5 sizes to content, and this sheet is a
          // handful of rows either way.
          backgroundComponent={SheetBackground}
          handleComponent={SheetHandle}
        >
          <View className="gap-3 p-page">
            {share ? (
              <View className="items-center gap-3">
                <Typography className="text-section text-foreground">{qrTitle}</Typography>
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
                <Typography className="text-section text-foreground" numberOfLines={1}>
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
          </View>
        </BottomSheet.Content>
      </BottomSheet.Portal>
    </BottomSheet>
  );
}

/** Unlabelled surface so VoiceOver does not announce a bare "BottomSheet". */
function SheetBackground() {
  return <View accessible={false} className="flex-1 rounded-t-3xl bg-surface" />;
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
      variant={destructive ? "danger-soft" : "tertiary"}
      onPress={onPress}
      accessibilityRole="button"
    >
      <Button.Label>{label}</Button.Label>
    </Button>
  );
}
