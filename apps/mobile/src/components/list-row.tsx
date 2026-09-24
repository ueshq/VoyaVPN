import { PressableFeedback } from "heroui-native/pressable-feedback";
import { Typography } from "heroui-native/text";
import type { ReactNode, Ref } from "react";
import { View, type AccessibilityActionEvent, type AccessibilityActionInfo, type AccessibilityRole, type AccessibilityState } from "react-native";

/**
 * One row of an inset grouped list: white rows on the grey canvas, rounded
 * where a group starts and ends, with a hairline between neighbours.
 *
 * A virtualized list cannot wrap its rows in a card, so each row draws its own
 * share of one: `first` rounds the top, `last` rounds the bottom and drops the
 * divider, and `inset` adds the page margin a card would have had. Inside a
 * `ListCard`, which does the wrapping itself, leave `inset` off.
 *
 * The rounding sits on the pressable itself because only that view clips: the
 * press highlight is an absolute layer inside it and would otherwise paint
 * square corners over a round card.
 */
export function ListRow({
  accessibilityActions,
  accessibilityLabel,
  accessibilityRole,
  accessibilityState,
  children,
  description,
  descriptionLines = 1,
  dimmed = false,
  first = false,
  inset = false,
  isDisabled,
  last = false,
  leading,
  onAccessibilityAction,
  onLongPress,
  onPress,
  ref,
  stacked = false,
  title,
  titleClassName = "text-foreground",
  titleLines,
  trailing,
}: {
  accessibilityActions?: readonly AccessibilityActionInfo[];
  accessibilityLabel?: string;
  accessibilityRole?: AccessibilityRole;
  accessibilityState?: AccessibilityState;
  /** Anything under the title and description, such as a failure line. */
  children?: ReactNode;
  description?: string;
  descriptionLines?: number;
  /** Shown but not in effect, such as a rule while global mode skips them all. */
  dimmed?: boolean;
  first?: boolean;
  inset?: boolean;
  isDisabled?: boolean;
  last?: boolean;
  /** An icon or badge of at most 28pt, before the text. */
  leading?: ReactNode;
  onAccessibilityAction?: (event: AccessibilityActionEvent) => void;
  onLongPress?: () => void;
  onPress?: () => void;
  /** Reaches the row only when it is pressable — the one kind focus returns to. */
  ref?: Ref<View>;
  /** Put `trailing` under the text instead of beside it, for narrow or large-text layouts. */
  stacked?: boolean;
  title: string;
  titleClassName?: string;
  titleLines?: number;
  trailing?: ReactNode;
}) {
  const shape = `bg-surface ${inset ? "mx-page" : ""} ${first ? "rounded-t-3xl" : ""} ${
    last ? "rounded-b-3xl" : ""
  } ${dimmed ? "opacity-40" : ""}`;

  const content = (
    <>
      <View className={`min-h-control gap-3 px-4 py-3 ${stacked ? "" : "flex-row items-center"}`}>
        <View className="min-w-0 flex-1 flex-row items-center gap-3">
          {leading ? <View className="w-7 items-center">{leading}</View> : null}
          <View className="min-w-0 flex-1 gap-0.5">
            <Typography className={`text-base ${titleClassName}`} numberOfLines={titleLines}>
              {title}
            </Typography>
            {description ? (
              <Typography className="text-sm text-subtle" numberOfLines={descriptionLines}>
                {description}
              </Typography>
            ) : null}
            {children}
          </View>
        </View>
        {trailing ? (
          <View className={stacked ? (leading ? "pl-10" : "") : "shrink-0 items-end"}>{trailing}</View>
        ) : null}
      </View>
      {last ? null : (
        <View
          accessible={false}
          className={`absolute bottom-0 right-0 h-hairline bg-border-subtle ${leading ? "left-14" : "left-4"}`}
        />
      )}
    </>
  );

  return onPress || onLongPress ? (
    <PressableFeedback
      ref={ref}
      animation="disable-all"
      className={shape}
      isDisabled={isDisabled}
      onPress={onPress}
      onLongPress={onLongPress}
      onAccessibilityAction={onAccessibilityAction}
      accessibilityActions={accessibilityActions}
      accessibilityLabel={accessibilityLabel}
      accessibilityRole={accessibilityRole ?? "button"}
      accessibilityState={accessibilityState}
    >
      <PressableFeedback.Highlight />
      {content}
    </PressableFeedback>
  ) : (
    <View
      className={shape}
      accessibilityLabel={accessibilityLabel}
      accessibilityRole={accessibilityRole}
      accessibilityState={accessibilityState}
    >
      {content}
    </View>
  );
}
