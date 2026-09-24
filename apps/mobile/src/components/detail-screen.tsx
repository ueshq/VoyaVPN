import { ScrollView } from "react-native";
import type { ReactNode } from "react";
import { useScreenInsets } from "./use-screen-insets";

export function DetailScreen({ children }: { children: ReactNode }) {
  const insets = useScreenInsets();
  return <ScrollView className="flex-1 bg-canvas" contentContainerClassName="gap-4 px-page" contentContainerStyle={insets}
    keyboardShouldPersistTaps="handled" keyboardDismissMode="on-drag" automaticallyAdjustKeyboardInsets>{children}</ScrollView>;
}
