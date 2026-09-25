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
 * A tone's foreground as a colour value, for the props that take one — an
 * SVG icon's `color` is not a style, so a class cannot reach it.
 */
export function useToneColor(tone: Tone) {
  const { color } = useResolveClassNames(TONE_TEXT[tone]);

  return typeof color === "string" ? color : undefined;
}
