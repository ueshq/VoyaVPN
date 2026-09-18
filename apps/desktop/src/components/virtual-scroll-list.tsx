import { useRef, type ComponentProps, type ReactNode } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";

import { cn } from "@voya/ui/lib/utils";
import { firstPaintVirtualItems } from "@/lib/virtual-list";

type VirtualScrollListProps<T> = Omit<ComponentProps<"div">, "children"> & {
  /** Estimated row height in px; rows are measured once rendered. */
  estimateSize: number;
  itemKey: (item: T) => string;
  items: readonly T[];
  renderItem: (item: T) => ReactNode;
};

/**
 * A scroll box that mounts only the rows in view, for pickers that list every
 * node or every running process.
 *
 * The virtualizer lives here, next to the element it scrolls, rather than in
 * the dialog that shows the list: a dialog's content mounts one render after
 * its parent, so a virtualizer held by the parent would never see its scroll
 * element attach.
 */
export function VirtualScrollList<T>({
  className,
  estimateSize,
  itemKey,
  items,
  renderItem,
  ...props
}: VirtualScrollListProps<T>) {
  const viewportRef = useRef<HTMLDivElement>(null);
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual exposes scroll helpers that React Compiler cannot memoize safely.
  const virtualizer = useVirtualizer({
    count: items.length,
    estimateSize: () => estimateSize,
    getItemKey: (index) => itemKey(items[index]!),
    getScrollElement: () => viewportRef.current,
    initialRect: { height: 224, width: 480 },
    overscan: 8,
    paddingEnd: 4,
    paddingStart: 4,
  });
  const rows = firstPaintVirtualItems(
    virtualizer.getVirtualItems(),
    items.length,
    estimateSize,
    12,
    (index) => itemKey(items[index]!),
  );

  return (
    <div className={cn("overflow-y-auto", className)} ref={viewportRef} {...props}>
      <div className="relative" style={{ height: virtualizer.getTotalSize() }}>
        {rows.map(({ index, key, start }) => {
          const item = items[index];
          if (item === undefined) return null;
          return (
            <div
              className="absolute inset-x-1"
              data-index={index}
              key={key}
              ref={virtualizer.measureElement}
              style={{ transform: `translateY(${start}px)` }}
            >
              {renderItem(item)}
            </div>
          );
        })}
      </div>
    </div>
  );
}
