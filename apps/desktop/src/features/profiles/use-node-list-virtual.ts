import { useEffect, useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import type { NodeListRow } from "@voya/features/profiles/node-list-rows";
import { firstPaintVirtualItems } from "@/lib/virtual-list";

/**
 * The desktop viewport over the shared row model: a virtualized window and the
 * scroll position that goes back to the top when the search changes.
 */
export function useNodeListVirtual(rows: NodeListRow[], search: string) {
  const viewportRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (viewportRef.current) viewportRef.current.scrollTop = 0;
  }, [search]);
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual exposes scroll helpers that React Compiler cannot memoize safely.
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    // Compact rows are about 56 px and group headers a little taller.
    estimateSize: () => 64,
    getItemKey: (index) => rows[index]!.key,
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 520, width: 1200 },
    overscan: 5,
  });
  const renderedRows = firstPaintVirtualItems(
    rowVirtualizer.getVirtualItems(),
    rows.length,
    64,
    15,
    (index) => rows[index]!.key,
  );

  return { renderedRows, rowVirtualizer, viewportRef };
}
