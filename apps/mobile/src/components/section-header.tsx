import { ChevronDown, ChevronRight } from "lucide-react-native";
import { PressableFeedback } from "heroui-native/pressable-feedback";
import { Typography } from "heroui-native/text";
import type { ReactNode } from "react";
import { View } from "react-native";

import { useToneColor } from "./tone";

/**
 * A section's heading, set outside the card it names.
 *
 * With `onToggle` it is also the section's disclosure: pressing it collapses
 * or expands what follows, and it says which with a chevron and with the
 * `expanded` state a screen reader announces.
 */
export function SectionHeader({
  detail,
  expanded,
  onToggle,
  title,
  trailing,
}: {
  /** A short fact beside the title, such as a count. */
  detail?: string;
  expanded?: boolean;
  onToggle?: () => void;
  title: string;
  trailing?: ReactNode;
}) {
  const chevronColor = useToneColor("neutral");
  const heading = (
    <View className="min-w-0 flex-1 flex-row items-center gap-1.5">
      {onToggle ? (
        expanded ? (
          <ChevronDown size={18} color={chevronColor} accessible={false} />
        ) : (
          <ChevronRight size={18} color={chevronColor} accessible={false} />
        )
      ) : null}
      <Typography
        numberOfLines={2}
        maxFontSizeMultiplier={2}
        className="shrink text-xl font-semibold text-foreground"
      >
        {title}
      </Typography>
      {detail ? (
        <Typography numberOfLines={1} className="shrink-0 text-sm text-subtle">
          {detail}
        </Typography>
      ) : null}
    </View>
  );

  return (
    <View className="min-h-control flex-row items-center justify-between gap-3">
      {onToggle ? (
        <PressableFeedback
          animation="disable-all"
          className="min-h-12 min-w-0 flex-1 justify-center rounded-xl"
          onPress={onToggle}
          accessibilityRole="button"
          accessibilityState={{ expanded }}
        >
          <PressableFeedback.Highlight />
          {heading}
        </PressableFeedback>
      ) : (
        <View accessibilityRole="header" accessible className="min-w-0 flex-1">
          {heading}
        </View>
      )}
      {trailing}
    </View>
  );
}
