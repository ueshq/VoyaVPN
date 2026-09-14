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

export function profileNameWithoutFlag(name: string) {
  const flag = name.match(/\p{Regional_Indicator}{2}/u)?.[0];
  return flag ? name.replace(flag, "").trim() || name : name;
}

const REGIONAL_INDICATOR_A = 0x1f1e6;

/**
 * The country a flag emoji in a node name points at ("🇯🇵 Tokyo" → "JP"). It
 * is only a provisional hint: a measured country always takes precedence.
 */
export function profileFlagCountryCode(name: string | null | undefined) {
  const flag = name?.match(/\p{Regional_Indicator}{2}/u)?.[0];
  if (!flag) return null;
  return String.fromCharCode(
    ...Array.from(flag, (letter) => (letter.codePointAt(0) ?? REGIONAL_INDICATOR_A) - REGIONAL_INDICATOR_A + 65),
  );
}
