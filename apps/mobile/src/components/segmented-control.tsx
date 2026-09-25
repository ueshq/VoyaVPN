import { Tabs } from "heroui-native/tabs";

/**
 * A choice of one among a few: HeroUI's primary `Tabs`, a track with the
 * chosen segment raised.
 *
 * Each segment overrides the trigger's `role="tab"` with `button`. React
 * Native gives `tab` no trait at all on iOS, so VoiceOver would not announce
 * the segment as something to press, and XCUITest would not find it among
 * `buttons`. The trigger still marks the chosen segment `selected`, which is
 * what Testing Library's `toBeSelected()` and XCUITest's `isSelected` read.
 * There are no panels here — the choice is the whole point — so nothing is
 * lost by not being a tab.
 *
 * Labels stay on one line, and a segment is as wide as its label plus an equal
 * share of what is left — equal widths truncated "Follow system" beside
 * "Light" and "Dark". When the labels do not fit, at large text sizes, the
 * track scrolls sideways.
 */
export function SegmentedControl<T extends string>({
  isDisabled,
  onChange,
  options,
  value,
}: {
  isDisabled?: boolean;
  onChange: (value: T) => void;
  options: readonly { label: string; value: T }[];
  value: T | null | undefined;
}) {
  return (
    <Tabs
      value={value ?? ""}
      onValueChange={(next) => {
        const option = options.find((candidate) => candidate.value === next);
        if (option) onChange(option.value);
      }}
    >
      <Tabs.List className="self-stretch">
        <Tabs.ScrollView>
          <Tabs.Indicator />
          {options.map((option) => (
            <Tabs.Trigger
              key={option.value}
              value={option.value}
              isDisabled={isDisabled}
              role="button"
              className="min-h-12 shrink-0 grow basis-auto"
            >
              <Tabs.Label numberOfLines={1}>{option.label}</Tabs.Label>
            </Tabs.Trigger>
          ))}
        </Tabs.ScrollView>
      </Tabs.List>
    </Tabs>
  );
}
