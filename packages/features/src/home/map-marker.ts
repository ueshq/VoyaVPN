import type { ProfileSummaryEntry } from "@voya/contracts";

import { entryCountry } from "../profiles/profile-display";

export type HomeMapMarker = {
  countryCode: string;
  /** Connected: where traffic leaves. Selected: where it would leave. */
  state: "connected" | "selected";
};

/**
 * Where Home's world map puts its one marker.
 *
 * While connected it shows where traffic actually leaves — the checked exit IP
 * wins over whatever the node claims — and before that, where the selected
 * node would take it. A policy group has no single country until one of its
 * members is running, and with no nodes at all there is nothing to mark.
 */
export function homeMapMarker({
  connected,
  exitCountryCode,
  groupEntry,
  hasNodes,
  isGroup,
  nodeEntry,
}: {
  connected: boolean;
  /** The country the exit-IP check reported, when it has reported one. */
  exitCountryCode: string | null | undefined;
  /** The entry behind the group member that is running, if any. */
  groupEntry: ProfileSummaryEntry | null;
  hasNodes: boolean;
  isGroup: boolean;
  nodeEntry: ProfileSummaryEntry | null;
}): HomeMapMarker | null {
  if (!hasNodes) return null;
  const countryCode = connected
    ? (exitCountryCode ?? entryCountry(isGroup ? groupEntry : nodeEntry))
    : isGroup
      ? null
      : entryCountry(nodeEntry);

  return countryCode ? { countryCode, state: connected ? "connected" : "selected" } : null;
}
