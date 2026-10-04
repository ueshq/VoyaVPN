import { PressableFeedback } from "heroui-native/pressable-feedback";
import { Typography } from "heroui-native/text";
import { useState, type ReactNode } from "react";
import { View } from "react-native";

import { ExpandChevron } from "./expand-chevron";

/**
 * A section that opens and closes under its title.
 *
 * It is a plain `PressableFeedback` row, not a HeroUI `Accordion` item: the
 * accordion wraps its trigger in a heading-role header, and on iOS that
 * wrapper becomes the element accessibility sees — VoiceOver announced the
 * title as a heading and an `AXPress` on it went nowhere. A row we lay out
 * ourselves exposes exactly one button carrying the `expanded` state a
 * screen reader announces. Collapsed content is not mounted at all.
 *
 * Pass `isExpanded` to control it — the DNS page keeps its fields open while
 * they hold unsaved edits.
 */
export function Disclosure({
  children,
  heading = false,
  isExpanded,
  onExpandedChange,
  title,
}: {
  children: ReactNode;
  /** Set the title as a section heading rather than as row text. */
  heading?: boolean;
  isExpanded?: boolean;
  onExpandedChange?: (isExpanded: boolean) => void;
  title: string;
}) {
  const [uncontrolled, setUncontrolled] = useState(false);
  const expanded = isExpanded ?? uncontrolled;

  return (
    <View className="gap-2">
      <PressableFeedback
        animation="disable-all"
        className="min-h-control flex-row items-center gap-2 px-0"
        onPress={() => {
          setUncontrolled(!expanded);
          onExpandedChange?.(!expanded);
        }}
        accessibilityRole="button"
        accessibilityState={{ expanded }}
      >
        <PressableFeedback.Highlight />
        <Typography
          maxFontSizeMultiplier={heading ? 2 : undefined}
          className={`min-w-0 flex-1 text-foreground ${heading ? "text-xl font-semibold" : "text-base"}`}
        >
          {title}
        </Typography>
        <ExpandChevron expanded={expanded} />
      </PressableFeedback>
      {expanded ? <View className="gap-4 px-0">{children}</View> : null}
    </View>
  );
}
