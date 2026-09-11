import type { KeyboardEvent } from "react";

/** Keep keyboard navigation usable when the destination row is not mounted yet. */
export function navigateVirtualList(event: KeyboardEvent<HTMLElement>, count: number, scrollToIndex: (index: number) => void, rowAttribute: string, focusSelector: string) {
  if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || !count) return;
  const target = event.target as HTMLElement;
  if (target.closest('[role="menu"]') || target.matches("input, textarea")) return;
  const index = Number(target.closest(`[${rowAttribute}]`)?.getAttribute(rowAttribute) ?? -1);
  let next: number;
  switch (event.key) {
    case "ArrowDown": next = Math.min(count - 1, index + 1); break;
    case "ArrowUp": next = Math.max(0, index - 1); break;
    case "Home": next = 0; break;
    case "End": next = count - 1; break;
    default: return;
  }
  event.preventDefault();
  const viewport = event.currentTarget;
  scrollToIndex(next);
  requestAnimationFrame(() => {
    viewport.querySelector<HTMLElement>(`[${rowAttribute}="${next}"] ${focusSelector}`)?.focus({ preventScroll: true });
  });
}
