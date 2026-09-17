import type { VirtualItem } from "@tanstack/react-virtual";

/**
 * TanStack Virtual reports no items until the scroll element is first
 * measured, which would blank a freshly opened list for a frame. Render a
 * fixed prefix by hand for that first paint; `rowKey` keeps the fallback rows
 * under the same React keys the measured rows will use.
 */
export function firstPaintVirtualItems(
  virtualItems: VirtualItem[],
  rowCount: number,
  rowHeight: number,
  fallbackCount: number,
  rowKey?: (index: number) => string | number,
): Array<Pick<VirtualItem, "index" | "key" | "start">> {
  if (virtualItems.length > 0) {
    return virtualItems;
  }
  return Array.from({ length: Math.min(fallbackCount, rowCount) }, (_, index) => ({
    index,
    key: rowKey ? rowKey(index) : index,
    start: index * rowHeight,
  }));
}
