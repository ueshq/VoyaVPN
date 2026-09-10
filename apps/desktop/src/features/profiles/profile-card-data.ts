import type { TranslationFunction } from "@voya/i18n";
import type { ProfileListEntry } from "@/ipc/bindings";
import { speedtestOutcomeText } from "@/ipc/messages";
import { formatDelay } from "@voya/utils/formatting";

export function profileLatency(item: ProfileListEntry, t: TranslationFunction) {
  const { delayMs, outcome } = item.metrics;
  return outcome && outcome !== "completed" ? speedtestOutcomeText(t, outcome) : formatDelay(delayMs) || "—";
}
