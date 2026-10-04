import { queryOptions } from "@tanstack/react-query";

import { profileDetailsQueryKey, profileShareQrQueryKey } from "./query-keys";
import { voyaCommands } from "./transport";

// Beside `queries` rather than in it: that module loads with the desktop
// shell, and these two are read only by dialogs and pages opened later.

/** One node in full; under the profiles root, so any node write refreshes it. */
export function profileDetailsQuery(indexId: string) {
  return queryOptions({
    queryKey: profileDetailsQueryKey(indexId),
    queryFn: () => voyaCommands().getProfile(indexId),
  });
}

/**
 * A share link rendered as a QR code. The link is the key, credentials and
 * all, so the entry goes the moment nothing shows it rather than sitting in
 * the cache; the rendering of a given link never goes stale.
 */
export function profileShareQrQuery(content: string) {
  return queryOptions({
    queryKey: profileShareQrQueryKey(content),
    queryFn: () => voyaCommands().generateQrCode(content),
    gcTime: 0,
    staleTime: Infinity,
  });
}
