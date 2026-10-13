import { Button } from "heroui-native/button";
import { Spinner } from "heroui-native/spinner";
import { Typography } from "heroui-native/text";
import { Power } from "lucide-react-native";
import { StyleSheet, View, useWindowDimensions } from "react-native";
import Svg, { Circle, Defs, RadialGradient, Stop } from "react-native-svg";

import { usePalette, useThemeShadow } from "~/components/use-palette";

/** An actual button above decorative SVG rings, so the whole disk is a touch target. */
export function ConnectionDial({
  connected,
  busy,
  disabled,
  label,
  stateLabel,
  onPress,
}: {
  connected: boolean;
  busy: boolean;
  disabled: boolean;
  label: string;
  stateLabel: string;
  onPress: () => void;
}) {
  const colors = usePalette();
  const shadow = useThemeShadow();
  const { width } = useWindowDimensions();
  const size = Math.min(320, width * 0.75);
  const disk = size * 0.7;
  const foreground = connected ? colors.onGreen : colors.power;
  return (
    <View style={{ width: size, height: size, alignItems: "center", justifyContent: "center" }}>
      <View
        pointerEvents="none"
        accessibilityElementsHidden
        importantForAccessibility="no-hide-descendants"
        style={StyleSheet.absoluteFill}
      >
        <Svg width="100%" height="100%" viewBox="0 0 320 320">
          {connected ? (
            <>
              <Circle cx="160" cy="160" r="158" fill={colors.halo} opacity={0.045} />
              <Circle cx="160" cy="160" r="139" fill={colors.halo} opacity={0.12} />
              <Circle cx="160" cy="160" r="124" fill={colors.halo} opacity={0.22} />
            </>
          ) : (
            <Circle
              cx="160"
              cy="160"
              r="151"
              fill="none"
              stroke={colors.dots}
              strokeWidth="3.5"
              strokeDasharray="0.1 22"
              strokeLinecap="round"
            />
          )}
        </Svg>
      </View>
      <Button
        testID="home-connect"
        variant="ghost"
        className="flex-col gap-3 p-5"
        style={{
          width: disk,
          height: disk,
          borderRadius: disk / 2,
          backgroundColor: connected ? colors.green : colors.dial,
          boxShadow: connected ? undefined : shadow.dial,
        }}
        isDisabled={disabled}
        onPress={onPress}
        accessibilityLabel={label}
        accessibilityValue={{ text: stateLabel }}
        accessibilityState={{ busy, disabled }}
      >
        <View
          pointerEvents="none"
          accessibilityElementsHidden
          importantForAccessibility="no-hide-descendants"
          style={StyleSheet.absoluteFill}
        >
          <Svg width="100%" height="100%" viewBox="0 0 224 224">
            <Defs>
              <RadialGradient id="connection-disk" cx="50%" cy="47%" r="53%">
                <Stop offset="0" stopColor={connected ? colors.green : colors.dial} />
                <Stop offset="0.82" stopColor={connected ? colors.green : colors.dial} />
                <Stop offset="1" stopColor={connected ? colors.greenEdge : colors.dialEdge} />
              </RadialGradient>
            </Defs>
            <Circle cx="112" cy="112" r="112" fill="url(#connection-disk)" />
          </Svg>
        </View>
        {busy ? (
          <Spinner size="lg" color={foreground} />
        ) : (
          <Power size={disk * 0.3} strokeWidth={2.4} color={foreground} accessible={false} />
        )}
        <Typography
          maxFontSizeMultiplier={1.5}
          numberOfLines={2}
          adjustsFontSizeToFit
          minimumFontScale={0.8}
          className="text-center text-xl"
          style={{ color: foreground }}
        >
          {label}
        </Typography>
      </Button>
    </View>
  );
}
