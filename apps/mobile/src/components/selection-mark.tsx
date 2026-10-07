import { Circle, CircleCheck } from "lucide-react-native";

import { useToneColor } from "./tone";

/**
 * The radio mark before a choosable row: a tap on the row selects it, and the
 * mark says which one is. Decoration only — the row's own text says it too.
 */
export function SelectionMark({ state }: { state: "inUse" | "none" | "selected" }) {
  const tone = state === "inUse" ? "connected" : state === "selected" ? "brand" : "neutral";
  const color = useToneColor(tone);

  return state === "none"
    ? <Circle size={22} color={color} strokeWidth={1.5} accessible={false} />
    : <CircleCheck size={22} color={color} strokeWidth={2} accessible={false} />;
}
