import { createNavigationContainerRef } from "@react-navigation/native";

import type { ShellTab } from "./tabs";

/**
 * The navigator, reachable from outside the tree.
 *
 * Deep links arrive on a backend event, not from a component, so the event
 * bridge needs a handle that does not depend on where it happens to be
 * mounted. `isReady()` is false before the container mounts and after it
 * unmounts, which is exactly when a navigation would throw.
 */
export const navigationRef = createNavigationContainerRef<Record<ShellTab, undefined>>();

export function navigateToTab(tab: ShellTab) {
  if (navigationRef.isReady()) {
    navigationRef.navigate(tab);
  }
}
