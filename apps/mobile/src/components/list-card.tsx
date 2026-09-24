import type { ReactNode } from "react";
import { View } from "react-native";

/**
 * A card that holds a fixed handful of `ListRow`s.
 *
 * For a list that is not virtualized — a settings group, a sheet's actions —
 * the card can simply wrap its rows; give the final row `last` so it draws no
 * divider. `overflow-hidden` is what rounds the first and last rows' press
 * highlight with the card.
 */
export function ListCard({ children }: { children: ReactNode }) {
  return <View className="overflow-hidden rounded-3xl bg-surface shadow-surface">{children}</View>;
}
