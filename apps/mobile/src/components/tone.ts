import { useResolveClassNames } from "uniwind";

/**
 * The few colour roles a badge or an icon can take.
 *
 * Every class is written out in full: Tailwind finds classes by scanning the
 * source, so a class assembled at runtime would never be generated.
 */
export type Tone = "brand" | "connected" | "danger" | "neutral" | "warning";

export const TONE_BACKGROUND = {
  brand: "bg-brand-tint",
  connected: "bg-connected/15",
  danger: "bg-danger-soft",
  neutral: "bg-surface-secondary",
  warning: "bg-warning-soft",
} as const satisfies Record<Tone, string>;

const TONE_TEXT = {
  brand: "text-brand",
  connected: "text-connected",
  danger: "text-danger-soft-foreground",
  neutral: "text-subtle",
  warning: "text-warning-soft-foreground",
} as const satisfies Record<Tone, string>;

/**
 * The colour a class resolves to, for the props that take a value — an SVG
 * fill or an icon's `color` is not a style, so a class cannot reach it.
 *
 * Resolved from the class rather than read as a theme variable: Uniwind keeps
 * only the variables some `className` actually uses, and asking for the class
 * is what puts its variable in the build. For the same reason the class must
 * be written out where this is called — Tailwind finds classes by scanning
 * the source.
 */
export function useClassColor(className: string, property: "backgroundColor" | "color" = "color") {
  const value = useResolveClassNames(className)[property];

  return typeof value === "string" ? value : undefined;
}

/** A tone's foreground as a colour value. */
export function useToneColor(tone: Tone) {
  return useClassColor(TONE_TEXT[tone]);
}
