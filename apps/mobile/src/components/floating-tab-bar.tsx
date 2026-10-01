import { BottomTabBarHeightCallbackContext, type BottomTabBarProps } from "@react-navigation/bottom-tabs";
import { CommonActions } from "@react-navigation/native";
import { Typography } from "heroui-native/text";
import { Svg, Defs, LinearGradient, Rect, Stop } from "react-native-svg";
import { useContext } from "react";
import { Platform, Pressable, View } from "react-native";
import { useResolveClassNames } from "uniwind";

import { CONTENT_INNER_WIDTH, useContentColumn } from "./content-column";
import { useToneColor } from "./tone";

/** How far above the pill scrolling content fades into the canvas. */
const FADE_HEIGHT = 20;

/**
 * The tab bar: one rounded surface floating above the content.
 *
 * It replaces the stock bar rather than restyling it, because the stock bar is
 * edge-to-edge by construction. What the stock bar did for testing and for
 * VoiceOver is kept item for item: each tab is a button carrying the
 * screen's `tabBarButtonTestID`, its label (`tabBarLabel`, else the screen's
 * `title`) as the accessibility label unless `tabBarAccessibilityLabel` says
 * otherwise, and `selected` while it is the current tab — which is what
 * XCUITest reads back as `isSelected`.
 *
 * The bar is absolutely positioned, so the navigator cannot measure it; the
 * pill's container reports its own height and `useScreenInsets` pads each
 * screen's content by that much. The fade strip above the pill is deliberately
 * outside that measurement: it is decoration for content that scrolls under
 * the floating pill, which dissolves into the canvas instead of being cut in
 * half by the pill's top edge.
 */
export function FloatingTabBar({ descriptors, insets, navigation, state }: BottomTabBarProps) {
  const reportHeight = useContext(BottomTabBarHeightCallbackContext);
  const activeColor = useToneColor("brand");
  const inactiveColor = useToneColor("neutral");
  // On an iPad the pill sits under the content column, edge to edge with its
  // cards, instead of spanning the window.
  const column = useContentColumn(CONTENT_INNER_WIDTH);
  // Resolved from the class rather than read as a variable: Uniwind keeps only
  // the theme variables some `className` actually uses, and an SVG fill is not
  // one. Asking for the class is what puts `--color-canvas` in the build.
  const { backgroundColor } = useResolveClassNames("bg-canvas");
  const canvas = typeof backgroundColor === "string" ? backgroundColor : "transparent";

  return (
    <View
      pointerEvents="box-none"
      className="absolute inset-x-0 bottom-0"
      // Android's three-button bar occupies the full inset; iOS leaves room
      // around its thin home indicator even with the floating design's offset.
      style={{ paddingBottom: Math.max(insets.bottom - (Platform.OS === "ios" ? 10 : 0), 12) }}
    >
      <View
        pointerEvents="none"
        style={{ height: FADE_HEIGHT }}
        // Decoration: nothing here is interactive or needs announcing.
        accessibilityElementsHidden
        importantForAccessibility="no-hide-descendants"
      >
        <Svg width="100%" height="100%" viewBox={`0 0 1 ${FADE_HEIGHT}`} preserveAspectRatio="none">
          <Defs>
            <LinearGradient id="voya-tab-fade" x1="0" y1="0" x2="0" y2={FADE_HEIGHT} gradientUnits="userSpaceOnUse">
              <Stop offset="0" stopColor={canvas} stopOpacity="0" />
              <Stop offset="1" stopColor={canvas} stopOpacity="1" />
            </LinearGradient>
          </Defs>
          <Rect x="0" y="0" width="1" height={FADE_HEIGHT} fill="url(#voya-tab-fade)" />
        </Svg>
      </View>
      <View className="px-page" onLayout={(event) => reportHeight?.(event.nativeEvent.layout.height)}>
        <View className="flex-row rounded-full border border-border-subtle bg-surface p-1.5 shadow-float" style={column}>
          {state.routes.map((route, index) => {
            const { options } = descriptors[route.key];
            const focused = state.index === index;
            const label = typeof options.tabBarLabel === "string"
              ? options.tabBarLabel
              : (options.title ?? route.name);

            return (
              <Pressable
                key={route.key}
                testID={options.tabBarButtonTestID}
                accessibilityRole="button"
                accessibilityLabel={options.tabBarAccessibilityLabel ?? label}
                accessibilityState={{ selected: focused }}
                // Tab labels do not grow with Dynamic Type — there is no room —
                // so a long press shows the large-content viewer, as UIKit's does.
                accessibilityShowsLargeContentViewer
                accessibilityLargeContentTitle={label}
                className={`min-h-control flex-1 items-center justify-center gap-0.5 rounded-full px-1 py-1.5 ${
                  focused ? "bg-surface-secondary" : ""
                }`}
                onPress={() => {
                  const event = navigation.emit({ canPreventDefault: true, target: route.key, type: "tabPress" });
                  if (!focused && !event.defaultPrevented) {
                    navigation.dispatch({ ...CommonActions.navigate(route), target: state.key });
                  }
                }}
                onLongPress={() => navigation.emit({ target: route.key, type: "tabLongPress" })}
              >
                {options.tabBarIcon?.({
                  color: (focused ? activeColor : inactiveColor) ?? "gray",
                  focused,
                  size: 22,
                })}
                <Typography
                  maxFontSizeMultiplier={1.5}
                  numberOfLines={2}
                  className={`text-xs font-medium ${focused ? "text-brand" : "text-subtle"}`}
                >
                  {label}
                </Typography>
              </Pressable>
            );
          })}
        </View>
      </View>
    </View>
  );
}
