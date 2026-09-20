import { useCallback, useSyncExternalStore } from "react";

import { appVisibilityAdapter } from "./platform";

/**
 * Whether the app is on screen, as a subscription.
 *
 * Reads through [`appVisibilityAdapter`] on every call rather than capturing it,
 * so a component that mounts before startup wiring still follows the real
 * adapter once it is registered.
 */
export function useAppVisible() {
  const subscribe = useCallback(
    (onChange: () => void) => appVisibilityAdapter().subscribe(onChange),
    [],
  );
  const getSnapshot = useCallback(() => appVisibilityAdapter().isVisible(), []);

  return useSyncExternalStore(subscribe, getSnapshot);
}
