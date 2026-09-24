import type { LucideIcon } from "lucide-react-native";
import { View } from "react-native";

import { TONE_BACKGROUND, useToneColor, type Tone } from "./tone";

const BADGE_SIZE = {
  lg: { box: "h-14 w-14", icon: 26 },
  md: { box: "h-10 w-10", icon: 20 },
  sm: { box: "h-7 w-7", icon: 16 },
} as const;

/**
 * An icon on a tinted disc. Decoration only: whatever it stands for is said
 * in the text beside it, so it is hidden from assistive technology.
 */
export function IconBadge({
  icon: Icon,
  size = "md",
  tone = "brand",
}: {
  icon: LucideIcon;
  size?: keyof typeof BADGE_SIZE;
  tone?: Tone;
}) {
  const color = useToneColor(tone);
  const { box, icon } = BADGE_SIZE[size];

  return (
    <View
      accessible={false}
      importantForAccessibility="no-hide-descendants"
      className={`items-center justify-center rounded-full ${box} ${TONE_BACKGROUND[tone]}`}
    >
      <Icon size={icon} color={color} />
    </View>
  );
}
