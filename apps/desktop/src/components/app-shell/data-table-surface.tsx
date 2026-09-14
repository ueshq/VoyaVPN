// Shared className vocabulary for the virtualized data tables. These are plain
// class strings (no JSX) so each screen keeps its own table markup and
// virtualizer geometry and only opts into the surface language: rows sit
// directly on the panel, a calm header, faint zebra stripes, and a blue hover
// state. Row heights are deliberately NOT encoded here — they live with the
// callers because each virtualizer's `estimateSize` depends on them.

// Sticky header band: an opaque panel-colored surface with uppercase muted
// labels — quieter than the body so the data leads.
export const dataTableHeader = "bg-surface-raised text-xs font-semibold uppercase text-muted-foreground";

// Zebra body rows. Even rows take a faint neutral tint that stays visible on
// the white light panel and the dark card alike; odd rows stay clear.
export const dataTableRowEven = "bg-surface-hovered/50";
export const dataTableRowOdd = "bg-transparent";

// Hover affordance for an interactive (non-selected) row: a light blue wash.
export const dataTableRowHover = "hover:bg-accent-blue-light";
