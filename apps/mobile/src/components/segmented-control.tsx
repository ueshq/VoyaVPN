import { PressableFeedback } from "heroui-native/pressable-feedback";
import { Typography } from "heroui-native/text";
import { View } from "react-native";

/**
 * A choice of one among a few, as a track with the chosen segment raised.
 *
 * Each segment is a button marked `selected` when chosen — the same contract
 * the separate choice buttons had, so Testing Library's `toBeSelected()` and
 * XCUITest's `isSelected` read it unchanged. Labels stay on one line, and a
 * segment is as wide as its label plus an equal share of what is left — equal
 * widths truncated "Follow system" beside "Light" and "Dark". With `stacked`,
 * for a narrow screen or large text, the segments stack instead.
 */
export function SegmentedControl<T extends string>({
  isDisabled,
  onChange,
  options,
  stacked = false,
  value,
}: {
  isDisabled?: boolean;
  onChange: (value: T) => void;
  options: readonly { label: string; value: T }[];
  stacked?: boolean;
  value: T | null | undefined;
}) {
  return (
    <View className={`gap-1 rounded-3xl bg-surface-secondary p-1 ${stacked ? "" : "flex-row"}`}>
      {options.map((option) => {
        const selected = option.value === value;

        return (
          <PressableFeedback
            key={option.value}
            animation="disable-all"
            className={`min-h-10 items-center justify-center rounded-full px-3 py-2 ${stacked ? "" : "grow basis-auto"} ${
              selected ? "bg-segment shadow-surface" : ""
            }`}
            isDisabled={isDisabled}
            onPress={() => onChange(option.value)}
            accessibilityRole="button"
            accessibilityState={{ selected }}
          >
            <PressableFeedback.Highlight />
            <Typography
              numberOfLines={stacked ? undefined : 1}
              className={`text-center text-sm font-medium ${selected ? "text-segment-foreground" : "text-subtle"}`}
            >
              {option.label}
            </Typography>
          </PressableFeedback>
        );
      })}
    </View>
  );
}
