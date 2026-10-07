import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "heroui-native/button";
import { ListGroup } from "heroui-native/list-group";
import { Typography } from "heroui-native/text";
import { LayoutArrowDown } from "lucide-react-native";
import { useRef, useState } from "react";
import { AccessibilityInfo, findNodeHandle, Modal, Pressable, View, type Text } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useContentColumn } from "~/components/content-column";
import { ListRow } from "~/components/list-row";
import { SelectionMark } from "~/components/selection-mark";

const ORDERS = [
  { latency: false, labelKey: "mobile.sortDefault" },
  { latency: true, labelKey: "mobile.sortLatency" },
] as const;

/**
 * The list's order, behind an icon: a sheet with a mark on the order in
 * effect. The choice is the shared persisted one, so the desktop's "Sort by
 * latency" and this agree.
 *
 * A native modal, as the node actions are, rather than HeroUI's `Menu`: that
 * popover drew its items but left every one of them out of the accessibility
 * tree, so VoiceOver could open the menu and choose nothing in it.
 */
export function NodeSortMenu({
  color,
  setSortByLatency,
  sortByLatency,
}: {
  color: string | undefined;
  setSortByLatency: (sortByLatency: boolean) => void;
  sortByLatency: boolean;
}) {
  const { t } = useI18n();
  const insets = useSafeAreaInsets();
  // On an iPad the sheet is a centred panel rather than the full width.
  const column = useContentColumn(480);
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<View>(null);
  const titleRef = useRef<Text>(null);
  const label = t("mobile.sortOrder");

  function focus(target: Parameters<typeof findNodeHandle>[0]) {
    const handle = findNodeHandle(target);
    if (handle != null) AccessibilityInfo.setAccessibilityFocus(handle);
  }

  function choose(latency: boolean) {
    setSortByLatency(latency);
    setOpen(false);
  }

  return (
    <>
      <Button
        ref={triggerRef}
        isIconOnly
        className="h-12 w-12 rounded-full bg-accent-soft"
        variant="secondary"
        accessibilityLabel={label}
        accessibilityValue={{ text: t(sortByLatency ? "mobile.sortLatency" : "mobile.sortDefault") }}
        onPress={() => setOpen(true)}
      >
        <LayoutArrowDown size={18} color={color} accessible={false} />
      </Button>
      <Modal
        visible={open}
        transparent
        animationType="fade"
        onRequestClose={() => setOpen(false)}
        onShow={() => focus(titleRef.current)}
        // Back to the button the sheet came from once it has gone.
        onDismiss={() => focus(triggerRef.current)}
      >
        <View style={{ flex: 1, justifyContent: "flex-end", paddingTop: insets.top + 16 }}>
          <Pressable accessible={false} className="absolute inset-0 bg-backdrop" onPress={() => setOpen(false)} />
          <View
            accessibilityViewIsModal
            className="gap-4 rounded-t-3xl bg-canvas p-5"
            style={[{ paddingBottom: Math.max(20, insets.bottom) }, column]}
          >
            <Typography
              ref={titleRef}
              accessibilityRole="header"
              maxFontSizeMultiplier={2}
              className="px-1 text-xl font-semibold text-foreground"
            >
              {label}
            </Typography>
            <ListGroup>
              {ORDERS.map((order, index) => (
                <ListRow
                  key={order.labelKey}
                  // Pinned rather than left to ListRow's default: reaching
                  // iOS as buttons is what the review found missing.
                  accessibilityRole="button"
                  accessibilityLabel={t(order.labelKey)}
                  accessibilityState={{ selected: order.latency === sortByLatency }}
                  last={index === ORDERS.length - 1}
                  title={t(order.labelKey)}
                  leading={<SelectionMark state={order.latency === sortByLatency ? "selected" : "none"} />}
                  onPress={() => choose(order.latency)}
                />
              ))}
            </ListGroup>
            <Button className="py-3" variant="tertiary" onPress={() => setOpen(false)} accessibilityRole="button">
              <Button.Label>{t("actions.close")}</Button.Label>
            </Button>
          </View>
        </View>
      </Modal>
    </>
  );
}
