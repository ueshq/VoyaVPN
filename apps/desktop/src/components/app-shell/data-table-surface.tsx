// Shared className vocabulary for the virtualized data tables and the node
// list. These are plain class strings (no JSX) so each screen keeps its own
// markup and virtualizer geometry and only opts into the surface language: rows
// sit directly on the panel, a calm header, hairline dividers, a blue hover
// state, and a stronger blue for a row that stays selected. Row heights are
// deliberately NOT encoded here — they live with the callers because each
// virtualizer's `estimateSize` depends on them.

// Sticky header band: an opaque panel-colored surface with small muted labels
// in sentence case (all caps only ever changed Latin text) — quieter than the
// body so the data leads.
export const dataTableHeader = "bg-surface-raised text-xs font-semibold text-muted-foreground";

// Body rows stay on the panel color and are separated by a hairline. A neutral
// zebra tint is the same gray family as the light page canvas, so tinted rows
// read as holes in the white panel; the last row draws no line.
export const dataTableRowDivider = "border-b border-border-subtle";

// Hover affordance for an interactive (non-selected) row: a light blue wash.
export const dataTableRowHover = "hover:bg-accent-blue-light";

// A row that stays selected (the node a connection uses): one step stronger
// than the hover wash. Selected rows take this instead of the hover class, so
// pointing at them does not change their color.
export const dataTableRowSelected = "bg-accent-blue-bg";
