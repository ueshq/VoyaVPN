import { ChevronLeft } from "lucide-react-native";
import { Pressable } from "react-native";
import { useI18n } from "@voya/i18n/use-i18n";

import { useToneColor } from "./tone";

/**
 * The stack's back chevron, drawn here instead of taken from the native
 * navigation bar.
 *
 * react-native-screens' UIKit back button is a native element no React Native
 * accessibility prop can reach, and it never showed up in the simulator's
 * accessibility tree at all — so VoiceOver coverage could not be tested or
 * guaranteed. Drawing the button keeps it a plain `Pressable` with a label.
 * The edge-swipe back gesture and `usePreventRemove` both stay, because they
 * hook navigation events rather than this button.
 */
export function StackBackButton({ onPress }: { onPress: () => void }) {
  const { t } = useI18n();
  const color = useToneColor("brand");
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={t("actions.back")}
      hitSlop={8}
      className="min-h-control w-12 items-center justify-center pl-1"
      onPress={onPress}
    >
      <ChevronLeft size={26} color={color} accessible={false} />
    </Pressable>
  );
}
