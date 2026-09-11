import type { TranslationFunction } from "@voya/i18n";
import type { ProfileListEntry, ProfileTransport } from "@/ipc/bindings";
import { speedtestOutcomeText } from "@/ipc/messages";
import { formatDelay } from "@voya/utils/formatting";

export function profileLatency(item: ProfileListEntry, t: TranslationFunction) {
  const { delayMs, outcome } = item.metrics;
  return outcome && outcome !== "completed" ? speedtestOutcomeText(t, outcome) : formatDelay(delayMs) || "—";
}

export function profileTransportName(transport: ProfileTransport | null) {
  switch (transport?.kind) {
    case "websocket": return "ws";
    case "httpUpgrade": return "httpupgrade";
    case "http2": return "h2";
    default: return transport?.kind ?? "tcp";
  }
}
