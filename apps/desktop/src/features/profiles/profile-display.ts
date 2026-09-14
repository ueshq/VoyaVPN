import type { TranslationFunction } from "@voya/i18n";
import type { ProfileListEntry, ProfileTransport } from "@/ipc/bindings";
import { speedtestOutcomeText } from "@/ipc/messages";
import { formatDelay } from "@voya/utils/formatting";

export function profileLatency(item: ProfileListEntry, t: TranslationFunction) {
  const { delayMs, outcome } = item.metrics;
  return outcome && outcome !== "completed" ? speedtestOutcomeText(t, outcome) : delayMs > 0 ? formatDelay(delayMs) : t("panes.profiles.card.untested");
}

export type LatencyTone = "good" | "fair" | "poor" | "unknown";

/** Slow but reachable nodes are a warning; only failed tests use danger. */
export function profileLatencyTone(item: ProfileListEntry): LatencyTone {
  const { delayMs, outcome } = item.metrics;
  if (outcome && outcome !== "completed") return "poor";
  if (!delayMs || delayMs <= 0) return "unknown";
  return delayMs < 150 ? "good" : "fair";
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

/** A node's name without its flag; a node without a name goes by its id. */
export function profileMemberName(remarks: string, id: string) {
  return profileNameWithoutFlag(remarks) || id;
}

/** The name a node is listed under; one without a name says it is untitled. */
export function profileTitle(remarks: string, t: TranslationFunction) {
  return remarks || t("panes.profiles.untitled");
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

/** A measured country first; a flag in the node name is only a provisional hint. */
export function entryCountry(entry: ProfileListEntry | null | undefined) {
  return entry ? (entry.metrics.countryCode ?? profileFlagCountryCode(entry.profile.remarks)) : null;
}
