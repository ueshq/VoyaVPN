import { createContext, useContext } from "react";

/**
 * Whether the screen a hook runs under is the one the user is looking at.
 *
 * On the desktop a screen that is not showing is not mounted, so nothing has
 * to ask. A phone keeps its tabs mounted behind the one in front, and behind
 * any page pushed over them; what they poll and what they tick would carry on
 * for nobody. Its shell provides this per tab, and the default — active —
 * is every other caller's answer.
 */
export const ScreenActiveContext = createContext(true);

export function useScreenActive() {
  return useContext(ScreenActiveContext);
}
