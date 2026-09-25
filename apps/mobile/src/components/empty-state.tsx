import type { LucideIcon } from "lucide-react-native";
import { Surface } from "heroui-native/surface";
import { Typography } from "heroui-native/text";
import type { ReactNode } from "react";
import { View } from "react-native";

import { IconBadge } from "./icon-badge";
import type { Tone } from "./tone";

/** The cluster's tones, left to right: one quiet disc framing two coloured ones. */
const CLUSTER_TONES: readonly Tone[] = ["neutral", "brand", "connected"];

/**
 * What a screen shows instead of an empty list: a HeroUI `Surface` with a small
 * cluster of icons, what is missing, how to fill it, and at most one way to do so.
 */
export function EmptyState({
  action,
  description,
  icons,
  title,
}: {
  action?: ReactNode;
  description?: string;
  /** One icon, or a few for the overlapping cluster. */
  icons: readonly LucideIcon[];
  title: string;
}) {
  return (
    <Surface className="items-center gap-4 px-6 py-8">
      <View className="flex-row" accessible={false}>
        {icons.map((icon, index) => (
          <View
            key={index}
            // A white ring separates overlapping discs, as on a stack of avatars.
            className={`rounded-full border-4 border-surface ${index > 0 ? "-ml-4" : ""}`}
          >
            <IconBadge
              icon={icon}
              size="lg"
              tone={icons.length === 1 ? "neutral" : (CLUSTER_TONES[index] ?? "neutral")}
            />
          </View>
        ))}
      </View>
      <View className="items-center gap-2">
        <Typography maxFontSizeMultiplier={2} className="text-center text-xl font-semibold text-foreground">
          {title}
        </Typography>
        {description ? (
          <Typography className="text-center text-base text-subtle">{description}</Typography>
        ) : null}
      </View>
      {action ? <View className="w-full">{action}</View> : null}
    </Surface>
  );
}
