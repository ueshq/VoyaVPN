import { useRef } from "react";

import { useNodeSelection } from "@voya/features/profiles/use-node-selection";

/**
 * The desktop's node-list selection: the shared hook plus the DOM search input
 * ref its clear button refocuses.
 */
export function useNodeGroups() {
  const searchRef = useRef<HTMLInputElement>(null);
  const selection = useNodeSelection(searchRef);
  return {
    ...selection,
    searchRef,
    // Desktop call sites name the collapse toggle `toggle`.
    toggle: selection.toggleGroup,
  };
}
