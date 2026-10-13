import { designTokens } from "@voya/design-tokens";
import { useUniwind } from "uniwind";

/** Shared colors for native navigation, SVG and non-class-based platform APIs. */
export function usePalette() {
  return designTokens.colors[useUniwind().theme === "dark" ? "dark" : "light"];
}

export function useThemeShadow() {
  return designTokens.shadow[useUniwind().theme === "dark" ? "dark" : "light"];
}
