import { ListGroup } from "heroui-native/list-group";
import { PressableFeedback } from "heroui-native/pressable-feedback";
import { Separator } from "heroui-native/separator";
import type { ReactNode, Ref } from "react";
import { View, type AccessibilityActionEvent, type AccessibilityActionInfo, type AccessibilityRole, type AccessibilityState } from "react-native";

/**
 * One row of an inset grouped list, built from HeroUI's `ListGroup` parts:
 * rounded where a group starts and ends, with a `Separator` between
 * neighbours.
 *
 * A virtualized list cannot wrap its rows in a `ListGroup`, so each row draws
 * its own share of one: `first` rounds the top, `last` rounds the bottom and
 * drops the divider, and `inset` adds the page margin the group would have
 * had. Inside a `ListGroup`, which does the wrapping itself, leave `inset` off.
 *
 * A pressable row follows HeroUI's own recipe: `PressableFeedback` takes the
 * press, the ref and the accessibility element, and the `ListGroup.Item`
 * inside it is disabled and not accessible, so the row stays one element. The
 * rounding sits on the pressable because only that view clips: the press
 * highlight is an absolute layer inside it and would otherwise paint square
 * corners over a round group.
 */
export function ListRow({
  accessibilityActions,
  accessibilityLabel,
  accessibilityRole,
  accessibilityState,
  chevron = false,
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
  testID,
  title,
  titleClassName,
  titleLines,
  trailing,
  trailingInteractive = false,
}: {
  accessibilityActions?: readonly AccessibilityActionInfo[];
  accessibilityLabel?: string;
  accessibilityRole?: AccessibilityRole;
  accessibilityState?: AccessibilityState;
  /** Show the disclosure chevron, for a row that opens another page. */
  chevron?: boolean;
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
  testID?: string;
  title: string;
  titleClassName?: string;
  titleLines?: number;
  trailing?: ReactNode;
  /** Keep a trailing switch/button outside the row's accessibility element. */
  trailingInteractive?: boolean;
}) {
  const shape = `bg-surface ${inset ? "mx-page" : ""} ${first ? "rounded-t-3xl" : ""} ${
    last ? "rounded-b-3xl" : ""
  } ${dimmed ? "opacity-40" : ""}`;

  const text = (
    <>
      {leading ? <ListGroup.ItemPrefix className="w-7 items-center">{leading}</ListGroup.ItemPrefix> : null}
      <ListGroup.ItemContent className="min-w-0 gap-0.5">
        <ListGroup.ItemTitle className={titleClassName} numberOfLines={titleLines}>{title}</ListGroup.ItemTitle>
        {description ? <ListGroup.ItemDescription numberOfLines={descriptionLines}>{description}</ListGroup.ItemDescription> : null}
        {children}
      </ListGroup.ItemContent>
    </>
  );
  const divider = last ? null : (
    <Separator accessible={false} className={`absolute bottom-0 right-0 ${leading ? "left-14" : "left-4"}`} />
  );
  const pressProps = {
    accessibilityActions,
    accessibilityLabel,
    accessibilityRole: accessibilityRole ?? "button",
    accessibilityState,
    isDisabled,
    onAccessibilityAction,
    onLongPress,
    onPress,
    ref,
    testID,
  } as const;

  if (trailingInteractive && (onPress || onLongPress)) {
    return <View className={`overflow-hidden ${shape}`}>
      <View className={stacked ? "" : "flex-row items-center"}>
        <PressableFeedback animation="disable-all" className="flex-1" {...pressProps}>
          <PressableFeedback.Highlight />
          <ListGroup.Item disabled accessible={false} className={stacked ? "" : "pr-2"}>{text}</ListGroup.Item>
        </PressableFeedback>
        <ListGroup.ItemSuffix className={stacked ? "px-4 pb-4" : "shrink-0 pr-4"}>{trailing}</ListGroup.ItemSuffix>
      </View>{divider}
    </View>;
  }

  const row = (
    <ListGroup.Item disabled accessible={false} className={stacked ? "flex-col items-stretch" : ""}>
      {stacked ? <View className="flex-row items-center gap-3">{text}</View> : text}
      {trailing ? (
        <ListGroup.ItemSuffix className={stacked ? (leading ? "pl-10" : "") : "shrink-0 items-end"}>{trailing}</ListGroup.ItemSuffix>
      ) : chevron ? <ListGroup.ItemSuffix /> : null}
    </ListGroup.Item>
  );

  return onPress || onLongPress ? (
    <PressableFeedback animation="disable-all" className={shape} {...pressProps}>
      <PressableFeedback.Highlight />
      {row}
      {divider}
    </PressableFeedback>
  ) : (
    <View
      className={shape}
      accessibilityLabel={accessibilityLabel}
      accessibilityRole={accessibilityRole}
      accessibilityState={accessibilityState}
    >
      {row}
      {divider}
    </View>
  );
}
