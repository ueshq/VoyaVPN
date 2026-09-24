import { Typography } from "heroui-native/text";
import type { ReactNode } from "react";
import { View } from "react-native";

/**
 * A screen's large title, left-aligned, scrolling with the content.
 *
 * It stands in for a navigation bar — there is none — so it carries the
 * header role VoiceOver uses to jump between screens' titles. `trailing` holds
 * the one page-level action a screen may have.
 */
export function PageHeader({ title, trailing }: { title: string; trailing?: ReactNode }) {
  return (
    <View className="flex-row items-center justify-between gap-3 pb-1 pt-4">
      <Typography
        accessibilityRole="header"
        // Headings grow with Dynamic Type but less than running text, as
        // UIKit's large titles do; the caps keep this the largest of them
        // (section headings stop at 2x, Home's state word at 1.6x).
        maxFontSizeMultiplier={1.6}
        className="min-w-0 flex-1 text-3xl font-bold text-foreground"
      >
        {title}
      </Typography>
      {trailing}
    </View>
  );
}
