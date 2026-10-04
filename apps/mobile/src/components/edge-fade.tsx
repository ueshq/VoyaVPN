import { View } from "react-native";
import { Svg, Defs, LinearGradient, Rect, Stop } from "react-native-svg";

import { useClassColor } from "./tone";

/**
 * A strip that dissolves whatever is under it into the surface beside it: the
 * content above the floating tab bar fades into the canvas, and a scrolling
 * card's edge fades into the card.
 *
 * `colorClassName` is the background class of that surface, written out at
 * the call site (see `useClassColor`). The strip fills the box its parent or
 * `className` gives it; `size` is only the gradient's length in points.
 */
export function EdgeFade({
  className,
  colorClassName,
  edge,
  size,
}: {
  className?: string;
  colorClassName: string;
  /** The edge the fade ends on, fully opaque. */
  edge: "bottom" | "right";
  size: number;
}) {
  const color = useClassColor(colorClassName, "backgroundColor") ?? "transparent";
  const down = edge === "bottom";
  const id = `voya-edge-fade-${edge}`;

  return (
    <View
      pointerEvents="none"
      // Decoration: nothing here is interactive or needs announcing.
      accessibilityElementsHidden
      importantForAccessibility="no-hide-descendants"
      className={className}
      style={down ? { height: size } : undefined}
    >
      <Svg
        width="100%"
        height="100%"
        viewBox={down ? `0 0 1 ${size}` : `0 0 ${size} 1`}
        preserveAspectRatio="none"
      >
        <Defs>
          <LinearGradient
            id={id}
            x1="0"
            y1="0"
            x2={down ? 0 : size}
            y2={down ? size : 0}
            gradientUnits="userSpaceOnUse"
          >
            <Stop offset="0" stopColor={color} stopOpacity="0" />
            <Stop offset="1" stopColor={color} stopOpacity="1" />
          </LinearGradient>
        </Defs>
        <Rect x="0" y="0" width={down ? 1 : size} height={down ? size : 1} fill={`url(#${id})`} />
      </Svg>
    </View>
  );
}
