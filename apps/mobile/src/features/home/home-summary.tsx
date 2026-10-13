import { designTokens } from "@voya/design-tokens";
import type { useHomeRuntime } from "@voya/features/home/use-home-runtime";
import { profileLatency, profileTitle } from "@voya/features/profiles/profile-display";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatDurationMs } from "@voya/utils/formatting";
import { Card } from "heroui-native/card";
import { Typography } from "heroui-native/text";
import { ChevronRight, Globe } from "lucide-react-native";
import { Platform, Pressable, View, useWindowDimensions } from "react-native";

import { openPage } from "~/app/navigation";
import { useConnectedDurationMs } from "./use-connected-duration";
import { usePalette, useThemeShadow } from "~/components/use-palette";

export function HomeSummary({ runtime }: { runtime: ReturnType<typeof useHomeRuntime> }) {
  const { t } = useI18n();
  const colors = usePalette();
  const shadow = useThemeShadow();
  const duration = useConnectedDurationMs();
  const { fontScale } = useWindowDimensions();
  const entry = runtime.activeGroup ? null : runtime.nodeEntry;
  const name =
    runtime.activeGroup?.group.name ?? (entry ? profileTitle(entry.profile.remarks, t) : t("home.noSelection"));
  const subtitle = runtime.activeGroup
    ? runtime.groupNow
      ? t("home.groupVia", { node: profileTitle(runtime.groupNow.remarks, t) })
      : t("home.chooseNode")
    : (entry?.profile.address ?? t("home.chooseNode"));
  const measured =
    entry && (!entry.metrics.outcome || entry.metrics.outcome === "completed") && entry.metrics.delayMs > 0;
  const delay = runtime.activeGroup ? runtime.groupNow?.delayMs : measured ? entry.metrics.delayMs : null;
  const latency = delay != null && delay > 0 ? String(delay) : entry ? profileLatency(entry, t) : "—";
  const hasDelay = delay != null && delay > 0;
  const elapsed = runtime.connected ? (duration == null ? "—" : formatDurationMs(duration)) : formatDurationMs(0);
  const cardStyle = { backgroundColor: colors.card, borderRadius: designTokens.radius.card, boxShadow: shadow.card };
  const numberStyle = {
    color: colors.ink,
    fontFamily: Platform.select(designTokens.nativeFont.statistic),
    fontVariant: ["tabular-nums"] as "tabular-nums"[],
  };
  const stacked = fontScale > 1.35;

  return (
    <View className="gap-3">
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={`${t("home.chooseNode")}, ${name}, ${subtitle}`}
        onPress={() => openPage("nodePicker")}
        style={({ pressed }) => ({ opacity: pressed ? 0.7 : 1 })}
      >
        <Card className="flex-row items-center gap-4 p-4" style={{ ...cardStyle, minHeight: 84 }}>
          <View
            className="items-center justify-center rounded-2xl"
            style={{ width: 58, height: 58, backgroundColor: colors.wash }}
          >
            <Globe size={30} strokeWidth={1.7} color={colors.icon} accessible={false} />
          </View>
          <View className="flex-1 gap-1">
            <Typography className="text-xs font-medium uppercase" style={{ color: colors.muted, letterSpacing: 1.3 }}>
              {t("mobile.currentNode")}
            </Typography>
            <Typography numberOfLines={1} className="text-base" style={{ color: colors.ink }}>
              {name}
            </Typography>
            <Typography
              numberOfLines={1}
              className="text-xs"
              style={{ color: colors.muted, fontFamily: Platform.select(designTokens.nativeFont.mono) }}
            >
              {subtitle}
            </Typography>
          </View>
          <ChevronRight size={20} color={colors.muted} accessible={false} />
        </Card>
      </Pressable>
      <Pressable
        testID="home-details"
        accessibilityRole="button"
        accessibilityLabel={t("activity.connectionDetails")}
        accessibilityValue={{
          text: `${t("home.latency")}: ${hasDelay ? `${latency} ${t("mobile.latencyUnit")}` : latency}, ${t("home.duration")}: ${elapsed}`,
        }}
        disabled={!runtime.connected}
        onPress={() => openPage("sessionDetails")}
        style={({ pressed }) => ({ opacity: pressed ? 0.7 : 1 })}
      >
        <Card
          className={stacked ? "gap-4 p-5" : "flex-row items-stretch py-5"}
          style={{ ...cardStyle, minHeight: 100 }}
        >
          <View className={stacked ? "gap-2" : "flex-1 gap-2 px-4"} style={{ paddingLeft: stacked ? 0 : 30 }}>
            <Typography className="text-sm" style={{ color: colors.muted }}>
              {t("mobile.latency")}
            </Typography>
            <Typography
              className={hasDelay ? "text-3xl font-semibold" : "text-base"}
              style={hasDelay ? numberStyle : { color: colors.ink }}
            >
              {latency}
              {hasDelay ? (
                <Typography
                  className="text-base font-normal"
                  style={{ color: colors.muted }}
                >{` ${t("mobile.latencyUnit")}`}</Typography>
              ) : null}
            </Typography>
          </View>
          <View
            style={{ backgroundColor: colors.divider, width: stacked ? "100%" : 1, height: stacked ? 1 : undefined }}
          />
          <View className={stacked ? "gap-2" : "flex-1 gap-2 px-4"} style={{ paddingLeft: stacked ? 0 : 30 }}>
            <Typography className="text-sm" style={{ color: colors.muted }}>
              {t("home.duration")}
            </Typography>
            <Typography
              testID="home-duration"
              numberOfLines={1}
              adjustsFontSizeToFit
              minimumFontScale={0.65}
              className="text-3xl font-semibold"
              style={numberStyle}
            >
              {elapsed}
            </Typography>
          </View>
        </Card>
      </Pressable>
    </View>
  );
}
