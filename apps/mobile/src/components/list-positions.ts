/**
 * Each item of a flat list, with whether it opens or closes the list — the two
 * facts a `ListRow` needs to draw its share of the card.
 */
export function withListPositions<T>(items: readonly T[]) {
  return items.map((item, index) => ({
    first: index === 0,
    item,
    last: index === items.length - 1,
  }));
}
