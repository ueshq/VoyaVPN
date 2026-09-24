import { BottomTabBarHeightContext } from "@react-navigation/bottom-tabs";
import { useContext } from "react";
import { useSafeAreaInsets } from "react-native-safe-area-context";

/**
 * The vertical padding a tab screen's scrolling content needs.
 *
 * There is no navigation header: each screen draws its own large title, so the
 * content starts under the status bar and the title scrolls away with it. The
 * tab bar floats over the bottom of the screen, so the content ends above it.
 *
 * The context is read directly rather than through `useBottomTabBarHeight`,
 * which throws outside a tab navigator — and a screen rendered on its own, as
 * every screen test does, simply has no bar to clear.
 */
export function useScreenInsets() {
  const safeArea = useSafeAreaInsets();
  const tabBar = useContext(BottomTabBarHeightContext) ?? 0;

  return { paddingBottom: Math.max(tabBar, safeArea.bottom) + 24, paddingTop: tabBar ? safeArea.top : 16 };
}
