import { useWindowDimensions, type ViewStyle } from "react-native";

/** From this window width up the content stops growing and is centred. */
const WIDE_WINDOW_WIDTH = 700;
const CONTENT_MAX_WIDTH = 640;
/** The `px-page` margin (`--spacing-page` in global.css), in points. */
const PAGE_MARGIN = 16;
/**
 * The width of what sits inside a page column's margins, such as its cards.
 * Something that floats over the page with its own margins, such as the tab
 * bar, takes this width to line up with them.
 */
export const CONTENT_INNER_WIDTH = CONTENT_MAX_WIDTH - 2 * PAGE_MARGIN;

/** The column style for a window of `width` points, or `undefined` on a phone. */
export function contentColumn(width: number, maxWidth = CONTENT_MAX_WIDTH): ViewStyle | undefined {
  return width >= WIDE_WINDOW_WIDTH ? { alignSelf: "center", maxWidth, width: "100%" } : undefined;
}

/**
 * The style that keeps content readable on an iPad: a centred column instead
 * of rows stretched across the whole window. No iPhone is this wide, so there
 * it is `undefined` and the layout is untouched. It follows the window, so an
 * iPad in a narrow split view gets the phone layout.
 */
export function useContentColumn(maxWidth?: number): ViewStyle | undefined {
  return contentColumn(useWindowDimensions().width, maxWidth);
}
