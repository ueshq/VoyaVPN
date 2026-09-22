import { WORLD_MAP_BOX, worldMapPosition } from "@voya/features/home/country-positions";
import type { HomeMapMarker } from "@voya/features/home/map-marker";
import { WORLD_MAP_LAND } from "@voya/features/home/world-map-land";
import { View } from "react-native";
import Svg, { Defs, LinearGradient, Path, Stop } from "react-native-svg";
import { useResolveClassNames } from "uniwind";

const VIEW_BOX = `0 0 ${WORLD_MAP_BOX.width} ${WORLD_MAP_BOX.height}`;

/**
 * Home's world map, with the exit country marked.
 *
 * The desktop paints the same outline as a CSS mask over a colour token, which
 * React Native has no equivalent for, so the land is filled with a gradient of
 * that token instead — the same fade the asset's own mask applies, top and
 * bottom. The geometry and the projection are shared, so a marker sits in the
 * same place on both.
 */
export function WorldMap({ marker }: { marker: HomeMapMarker | null }) {
  // Resolved from the class rather than read as a variable: Uniwind keeps only
  // the theme variables some `className` actually uses, and an SVG fill is not
  // one. Asking for the class is what puts `--color-map-land` in the build.
  const { backgroundColor } = useResolveClassNames("bg-map-land");
  const land = typeof backgroundColor === "string" ? backgroundColor : "transparent";
  const position = marker ? worldMapPosition(marker.countryCode) : null;

  return (
    <View
      className="w-full"
      style={{ aspectRatio: WORLD_MAP_BOX.width / WORLD_MAP_BOX.height }}
      // Decoration: everything it says is said in words by the rows below it.
      accessibilityElementsHidden
      importantForAccessibility="no-hide-descendants"
    >
      <Svg width="100%" height="100%" viewBox={VIEW_BOX}>
        <Defs>
          <LinearGradient
            id="voya-world-map-fade"
            x1="0"
            y1="0"
            x2="0"
            y2={WORLD_MAP_BOX.height}
            gradientUnits="userSpaceOnUse"
          >
            <Stop offset="0" stopColor={land} stopOpacity="0.85" />
            <Stop offset="0.55" stopColor={land} stopOpacity="1" />
            <Stop offset="1" stopColor={land} stopOpacity="0.4" />
          </LinearGradient>
        </Defs>
        <Path
          d={WORLD_MAP_LAND}
          fill="url(#voya-world-map-fade)"
          fillRule="evenodd"
          testID="world-map-land"
        />
      </Svg>
      {marker && position ? (
        <View
          // Half the dot's own size, so the percentage lands on its centre.
          className={`absolute -ml-1.5 -mt-1.5 h-3 w-3 rounded-full ${
            marker.state === "connected"
              ? "bg-connected"
              : "border-2 border-brand bg-canvas"
          }`}
          style={{ left: `${position.left}%`, top: `${position.top}%` }}
          testID={`world-map-marker-${marker.countryCode}`}
        />
      ) : null}
    </View>
  );
}
