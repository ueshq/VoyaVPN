import type { UseQueryResult } from "@tanstack/react-query";
import type { ConnectionIpResult } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n/core";

/**
 * The exit address as Home words it, on either app: the lookup's state while
 * there is no answer, then the address with its country. An answer without an
 * address says so. With nothing to look up — no core is connected — it is the
 * dash the other connection figures show.
 */
export function exitIpLabel(
  ipQuery: Pick<UseQueryResult<ConnectionIpResult>, "data" | "fetchStatus" | "isError">,
  t: TranslationFunction,
) {
  if (ipQuery.fetchStatus === "fetching") return t("home.checkIpChecking");
  if (ipQuery.isError) return t("home.checkIpFailed");
  const result = ipQuery.data;
  if (!result) return "—";

  return [result.ip ?? t("home.checkIpUnknown"), result.countryCode].filter(Boolean).join(" · ");
}
