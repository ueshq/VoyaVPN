import { SITE_ORIGIN } from "./site";

export const LOCALES = ["en", "zh-hans", "zh-hant"] as const;
export type Locale = (typeof LOCALES)[number];

const PAGES = ["home", "support", "privacy"] as const;
export type Page = (typeof PAGES)[number];

export type Route = { locale: Locale; page: Page };

/** BCP 47 tag for `<html lang>` and `hreflang`. */
export const HTML_LANG: Record<Locale, string> = {
  en: "en",
  "zh-hans": "zh-Hans",
  "zh-hant": "zh-Hant",
};

function localePrefix(locale: Locale): string {
  return locale === "en" ? "" : `/${locale}`;
}

/**
 * The public path of a page. English lives at the root so the App Store URLs
 * (/support, /privacy) are the canonical English pages, with no redirect.
 * Workers Assets serves `support.html` at `/support` and a directory's
 * `index.html` at its trailing-slash path.
 */
export function pathFor({ locale, page }: Route): string {
  const prefix = localePrefix(locale);
  return page === "home" ? `${prefix}/` : `${prefix}/${page}`;
}

export function urlFor(route: Route): string {
  return `${SITE_ORIGIN}${pathFor(route)}`;
}

/** The file under dist/ that serves {@link pathFor}. */
export function fileFor({ locale, page }: Route): string {
  const dir = locale === "en" ? "" : `${locale}/`;
  return `${dir}${page === "home" ? "index" : page}.html`;
}

/** The 404 page for a locale; Workers Assets serves the nearest one. */
export function notFoundFileFor(locale: Locale): string {
  return locale === "en" ? "404.html" : `${locale}/404.html`;
}

export const ROUTES: Route[] = LOCALES.flatMap((locale) =>
  PAGES.map((page) => ({ locale, page })),
);
