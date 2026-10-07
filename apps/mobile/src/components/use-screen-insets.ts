import { createContext, useContext } from "react";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { useContentColumn } from "./content-column";

/** Native tab screens already clear the system bars and the native tab bar. */
export const NativeTabSafeAreaContext = createContext(false);

/**
 * Scroll-content spacing and the centred iPad column. Native tab screens sit
 * inside a native SafeAreaView; stack pages already sit below their header
 * but still need the bottom system inset. Neither needs a measured tab height.
 */
export function useScreenInsets() {
  const safeArea = useSafeAreaInsets();
  const nativeTab = useContext(NativeTabSafeAreaContext);
  const column = useContentColumn();

  return { ...column, paddingBottom: (nativeTab ? 0 : safeArea.bottom) + 24, paddingTop: 16 };
}
