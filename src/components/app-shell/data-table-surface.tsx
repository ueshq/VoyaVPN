// Shared className vocabulary for the redesign's virtualized data tables. These
// are plain class strings (no JSX) so each screen keeps its own table markup and
// virtualizer geometry and only opts into the surface language: a sunken outer
// well, a calm raised header, and blue selection / hover states. Row heights are
// deliberately NOT encoded here — they live with the callers because the
// virtualizer `estimateSize` and the screen tests assert the exact 38px / 40px
// rows. Screens compose these with `cn(...)` alongside their own geometry.

// Outer well: the table rests in a sunken surface so populated rows read as
// raised content floating above it. Mirrors the previous `border bg-card` well
// minus the height/flex geometry the screen owns.
export const dataTableWell = "overflow-hidden rounded-md border bg-surface-sunken";

// Sticky header band: a restrained raised surface with uppercase muted labels —
// quieter than the body so the data leads.
export const dataTableHeader = "bg-surface-raised text-xs font-semibold uppercase text-muted-foreground";

// Zebra body rows. Even rows lift onto the raised surface; odd rows stay
// transparent so the sunken well reads through.
export const dataTableRowEven = "bg-surface-raised";
export const dataTableRowOdd = "bg-transparent";

// Hover affordance for an interactive (non-selected) row: a light blue wash.
export const dataTableRowHover = "hover:bg-accent-blue-light";

// Selected row: blue fill + blue text + an inset blue ring. Intentionally blue
// so it stays distinct from the green "active node" dot the screens render
// separately for the live profile/proxy.
export const dataTableRowSelected =
  "bg-accent-blue-light text-accent-blue ring-1 ring-inset ring-accent-blue/30";
