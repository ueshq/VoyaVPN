import { Globe2 } from "lucide-react";
import countries from "flag-icons/country.json";
import "flag-icons/css/flag-icons.min.css";

const supportedCodes = new Set(countries.map((country) => country.code).filter((code) => /^[a-z]{2}$/.test(code) && code !== "xx"));

/**
 * Callers pass the measured country when there is one, otherwise the
 * provisional hint from a flag in the node name (see `profileFlagCountryCode`).
 */
export function NodeCountryIcon({ countryCode }: { countryCode: string | null | undefined }) {
  const code = countryCode?.trim().toLowerCase();
  return code && supportedCodes.has(code)
    ? <span aria-hidden="true" className={`fi fi-${code} node-country-flag`} />
    : <Globe2 aria-hidden="true" className="size-6" strokeWidth={1.5} />;
}
