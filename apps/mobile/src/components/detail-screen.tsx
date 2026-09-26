import { ScrollView } from "react-native";
import type { ReactNode } from "react";
import { useScreenInsets } from "./use-screen-insets";

/**
 * The scrolling page every non-list screen is: canvas, page margins, and the
 * insets that clear the status bar and the floating tab bar. The keyboard
 * props matter only on a screen with a field and change nothing elsewhere.
 */
export function DetailScreen({
  accessibilityLabel,
  children,
  gap = "gap-4",
}: {
  accessibilityLabel?: string;
  children: ReactNode;
  /** Settings pages space whole sections, so they sit further apart. */
  gap?: "gap-4" | "gap-6";
}) {
  const insets = useScreenInsets();
  return <ScrollView accessibilityLabel={accessibilityLabel} className="flex-1 bg-canvas" contentContainerClassName={`${gap} px-page`} contentContainerStyle={insets}
    keyboardShouldPersistTaps="handled" keyboardDismissMode="on-drag" automaticallyAdjustKeyboardInsets>{children}</ScrollView>;
}
