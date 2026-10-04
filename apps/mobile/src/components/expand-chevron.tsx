import { ChevronDown, ChevronRight } from "lucide-react-native";

import { useToneColor } from "./tone";

/**
 * The chevron of a row that opens and closes: right while closed, down while
 * open. Decoration — the row itself carries the `expanded` state a screen
 * reader announces.
 */
export function ExpandChevron({ expanded }: { expanded: boolean | undefined }) {
  const color = useToneColor("neutral");
  const Icon = expanded ? ChevronDown : ChevronRight;

  return <Icon size={18} color={color} accessible={false} />;
}
