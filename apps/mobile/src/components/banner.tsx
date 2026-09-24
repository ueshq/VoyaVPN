import { CircleAlert, Info, TriangleAlert, type LucideIcon } from "lucide-react-native";
import { Typography } from "heroui-native/text";
import type { ReactNode } from "react";
import { View } from "react-native";

import { TONE_BACKGROUND, TONE_TEXT, useToneColor } from "./tone";

type BannerStatus = "danger" | "info" | "warning";

const STATUS = {
  danger: { icon: CircleAlert, text: TONE_TEXT.danger, tone: "danger" },
  info: { icon: Info, text: "text-foreground", tone: "brand" },
  warning: { icon: TriangleAlert, text: TONE_TEXT.warning, tone: "warning" },
} as const satisfies Record<BannerStatus, { icon: LucideIcon; text: string; tone: string }>;

/**
 * A message about the screen it sits on — a failure, a warning, the outcome of
 * an action — on a tinted panel with an icon, so it reads as a message rather
 * than as more of the page. `action` is at most one small button, such as a
 * retry.
 */
export function Banner({
  action,
  liveRegion = false,
  message,
  status,
}: {
  action?: ReactNode;
  /** Announce changes to VoiceOver/TalkBack, for outcomes that arrive later. */
  liveRegion?: boolean;
  message: string;
  status: BannerStatus;
}) {
  const { icon: Icon, text, tone } = STATUS[status];
  const color = useToneColor(tone);

  return (
    <View className={`flex-row gap-3 rounded-2xl px-4 py-3 ${TONE_BACKGROUND[tone]}`}>
      <View className="pt-0.5" accessible={false}>
        <Icon size={18} color={color} />
      </View>
      <View className="min-w-0 flex-1 items-start gap-2">
        <Typography
          accessibilityLiveRegion={liveRegion ? "polite" : undefined}
          className={`text-sm ${text}`}
        >
          {message}
        </Typography>
        {action}
      </View>
    </View>
  );
}
