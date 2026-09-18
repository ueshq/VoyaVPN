import { Globe2 } from "lucide-react";

// Only the 4x3 artwork of plain two-letter codes is reachable: a node's country
// is an ISO 3166-1 code, and nothing renders the square (`fis`) variant or the
// subdivision flags (`gb-eng`, `es-ct`, …). Globbing just those, instead of
// importing the flag-icons stylesheet, keeps the ~290 other SVGs out of the
// package and the flag rules out of the startup CSS.
const flagUrls = import.meta.glob<string>(
  ["/node_modules/flag-icons/flags/4x3/??.svg", "!**/xx.svg"],
  { eager: true, import: "default", query: "?url" },
);

const flagUrlByCode = new Map(
  Object.entries(flagUrls).map(([path, url]) => [path.slice(-"xx.svg".length, -".svg".length), url]),
);

/**
 * Callers pass the measured country when there is one, otherwise the
 * provisional hint from a flag in the node name (see `profileFlagCountryCode`).
 */
export function NodeCountryIcon({ countryCode }: { countryCode: string | null | undefined }) {
  const code = countryCode?.trim().toLowerCase();
  const flagUrl = code ? flagUrlByCode.get(code) : undefined;
  return flagUrl
    ? (
      <span
        aria-hidden="true"
        className={`fi fi-${code} node-country-flag`}
        style={{ backgroundImage: `url("${flagUrl}")` }}
      />
    )
    : <Globe2 aria-hidden="true" className="size-6" strokeWidth={1.5} />;
}
