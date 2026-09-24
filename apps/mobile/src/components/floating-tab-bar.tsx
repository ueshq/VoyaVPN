import { BottomTabBarHeightCallbackContext, type BottomTabBarProps } from "@react-navigation/bottom-tabs";
import { CommonActions } from "@react-navigation/native";
import { Typography } from "heroui-native/text";
import { useContext } from "react";
import { Pressable, View } from "react-native";

import { useToneColor } from "./tone";

/**
 * The tab bar: one rounded surface floating above the content.
 *
 * It replaces the stock bar rather than restyling it, because the stock bar is
 * edge-to-edge by construction. What the stock bar did for testing and for
 * VoiceOver is kept item for item: each tab is a button carrying the
 * screen's `tabBarButtonTestID`, its full title as the accessibility label and
 * `selected` while it is the current tab — which is what XCUITest reads back
 * as `isSelected`.
 *
 * The bar is absolutely positioned, so the navigator cannot measure it; it
 * reports its own height instead, and `useScreenInsets` pads each screen's
 * content by that much.
 */
export function FloatingTabBar({ descriptors, insets, navigation, state }: BottomTabBarProps) {
  const reportHeight = useContext(BottomTabBarHeightCallbackContext);
  const activeColor = useToneColor("brand");
  const inactiveColor = useToneColor("neutral");

  return (
    <View
      pointerEvents="box-none"
      className="absolute inset-x-0 bottom-0 px-page"
      // The home indicator is a gesture area, not a keep-out zone: iOS's own
      // floating bars sit partly over it.
      style={{ paddingBottom: Math.max(insets.bottom - 10, 12) }}
      onLayout={(event) => reportHeight?.(event.nativeEvent.layout.height)}
    >
      <View className="flex-row rounded-full border border-border-subtle bg-surface p-1.5 shadow-float">
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
              accessibilityLabel={options.tabBarAccessibilityLabel}
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
                allowFontScaling={false}
                numberOfLines={1}
                className={`text-[11px] font-medium ${focused ? "text-brand" : "text-subtle"}`}
              >
                {label}
              </Typography>
            </Pressable>
          );
        })}
      </View>
    </View>
  );
}
