import { Platform } from "react-native";

/**
 * The monospace face log lines and raw JSON render in.
 *
 * Tailwind's `font-mono` is a CSS stack, which React Native cannot apply — a
 * native `fontFamily` must be one real font per platform.
 */
export const MONO_FONT = Platform.select({ android: "monospace", default: "Menlo" });
