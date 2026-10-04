import {
  BottomTabBarHeightCallbackContext,
  type BottomTabBarProps,
} from "@react-navigation/bottom-tabs";
import { CommonActions } from "@react-navigation/native";
import { Typography } from "heroui-native/text";
import { useContext } from "react";
import { Platform, Pressable, View } from "react-native";

import { CONTENT_INNER_WIDTH, useContentColumn } from "./content-column";
import { EdgeFade } from "./edge-fade";
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
 * column holding the fade strip and the pill reports its combined height, and
 * `useScreenInsets` pads each screen's content by that much. The fade must be
 * inside the measurement: content scrolls under it, so padding that ignored it
 * left every screen's last few points resting behind the fade — half-read
 * text, untappable buttons. The fade itself stays non-interactive decoration
 * that dissolves content into the canvas instead of cutting it in half at the
 * pill's top edge.
 */
export function FloatingTabBar({
  descriptors,
  insets,
  navigation,
  state,
}: BottomTabBarProps) {
  const reportHeight = useContext(BottomTabBarHeightCallbackContext);
  const activeColor = useToneColor("brand");
  const inactiveColor = useToneColor("neutral");
  // On an iPad the pill sits under the content column, edge to edge with its
  // cards, instead of spanning the window.
  const column = useContentColumn(CONTENT_INNER_WIDTH);
  return (
    // The whole bar reports its height — fade strip, pill and the bottom
    // spacer below them: `useScreenInsets` pads every screen's content by
    // what this View measures, and anything the measurement skipped ended
    // up resting under the bar (the fade cost the Rules footer its last
    // 20pt; the spacer still cost every fits-one-screen page the same way).
    <View
      pointerEvents="box-none"
      className="absolute inset-x-0 bottom-0"
      // Android's three-button bar occupies the full inset; iOS leaves room
      // around its thin home indicator even with the floating design's offset.
      style={{
        paddingBottom: Math.max(
          insets.bottom - (Platform.OS === "ios" ? 10 : 0),
          12,
        ),
      }}
      onLayout={(event) => reportHeight?.(event.nativeEvent.layout.height)}
    >
      <View>
        <EdgeFade edge="bottom" size={FADE_HEIGHT} colorClassName="bg-canvas" />
        <View className="px-page">
          <View
            className="flex-row rounded-full border border-border-subtle bg-surface p-1.5 shadow-float"
            style={column}
          >
            {state.routes.map((route, index) => {
              const { options } = descriptors[route.key];
              const focused = state.index === index;
              const label =
                typeof options.tabBarLabel === "string"
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
                    const event = navigation.emit({
                      canPreventDefault: true,
                      target: route.key,
                      type: "tabPress",
                    });
                    if (!focused && !event.defaultPrevented) {
                      navigation.dispatch({
                        ...CommonActions.navigate(route),
                        target: state.key,
                      });
                    }
                  }}
                  onLongPress={() =>
                    navigation.emit({ target: route.key, type: "tabLongPress" })
                  }
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
    </View>
  );
}
