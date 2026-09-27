import type { Locale } from "../routes";
import { en } from "./en";
import type { SiteCopy } from "./types";
import { zhHans } from "./zh-hans";
import { zhHant } from "./zh-hant";

export const COPY: Record<Locale, SiteCopy> = {
  en,
  "zh-hans": zhHans,
  "zh-hant": zhHant,
};
