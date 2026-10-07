import type { ReactNode } from "react";
import { COPY } from "./content";
import { HTML_LANG, LOCALES, type Locale, type Page, pathFor, urlFor } from "./routes";
import { SUPPORT_EMAIL } from "./site";
import { Inline } from "./text";

const SHORT_LANGUAGE_NAME: Record<Locale, string> = {
  en: "EN",
  "zh-hans": "简体",
  "zh-hant": "繁體",
};

export const CONTAINER = "mx-auto w-full max-w-6xl px-4 sm:px-6";

type DocumentProps = {
  locale: Locale;
  /** `null` for the 404 page, which has no canonical URL or alternates. */
  page: Page | null;
  title: string;
  description: string;
  css: string;
  children: ReactNode;
};

export function Document({ locale, page, title, description, css, children }: DocumentProps) {
  const copy = COPY[locale];
  return (
    <html lang={HTML_LANG[locale]}>
      <head>
        <meta charSet="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>{title}</title>
        <meta name="description" content={description} />
        <meta name="color-scheme" content="light dark" />
        <meta name="theme-color" content="#ffffff" media="(prefers-color-scheme: light)" />
        <meta name="theme-color" content="#0d1117" media="(prefers-color-scheme: dark)" />
        {page === null ? (
          <meta name="robots" content="noindex" />
        ) : (
          <>
            <link rel="canonical" href={urlFor({ locale, page })} />
            {LOCALES.map((other) => (
              <link key={other} rel="alternate" hrefLang={HTML_LANG[other]} href={urlFor({ locale: other, page })} />
            ))}
            <link rel="alternate" hrefLang="x-default" href={urlFor({ locale: "en", page })} />
            <meta property="og:type" content="website" />
            <meta property="og:site_name" content="VoyaVPN" />
            <meta property="og:title" content={title} />
            <meta property="og:description" content={description} />
            <meta property="og:url" content={urlFor({ locale, page })} />
          </>
        )}
        <link rel="icon" href="/favicon.svg" type="image/svg+xml" />
        <link rel="apple-touch-icon" href="/apple-touch-icon.png" />
        <link rel="stylesheet" href={css} />
      </head>
      <body className="min-h-dvh antialiased">
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:absolute focus:start-4 focus:top-3 focus:rounded-md focus:bg-canvas focus:px-3 focus:py-2 focus:text-sm"
        >
          {copy.nav.skip}
        </a>
        <Header locale={locale} page={page} />
        <main id="main">{children}</main>
        <Footer locale={locale} />
      </body>
    </html>
  );
}

function Header({ locale, page }: { locale: Locale; page: Page | null }) {
  const copy = COPY[locale];
  const link = (target: Page) =>
    `rounded-md px-2 py-1.5 text-sm hover:text-fg ${page === target ? "font-medium text-fg" : "text-fg-muted"}`;
  return (
    <header className="border-b border-line-muted">
      <div className={`${CONTAINER} flex h-16 items-center gap-2`}>
        <a href={pathFor({ locale, page: "home" })} className="me-auto flex items-center gap-2.5 rounded-md">
          <img src="/favicon.svg" alt="" width={28} height={28} className="size-7 rounded-[7px]" />
          <span className="font-semibold tracking-tight">VoyaVPN</span>
        </a>
        <nav className="flex items-center gap-1">
          <a
            href={pathFor({ locale, page: "support" })}
            className={link("support")}
            aria-current={page === "support" ? "page" : undefined}
          >
            {copy.nav.support}
          </a>
          <a
            href={pathFor({ locale, page: "privacy" })}
            className={link("privacy")}
            aria-current={page === "privacy" ? "page" : undefined}
          >
            {copy.nav.privacy}
          </a>
        </nav>
        <LanguageSwitch locale={locale} page={page ?? "home"} />
      </div>
    </header>
  );
}

function LanguageSwitch({ locale, page }: { locale: Locale; page: Page }) {
  return (
    <nav aria-label={COPY[locale].nav.languages} className="ms-1 border-s border-line-muted ps-2">
      <ul className="flex items-center gap-0.5">
        {LOCALES.map((other) => (
          <li key={other}>
            <a
              href={pathFor({ locale: other, page })}
              hrefLang={HTML_LANG[other]}
              lang={HTML_LANG[other]}
              title={COPY[other].languageName}
              aria-current={other === locale ? "true" : undefined}
              className={`rounded-md px-1.5 py-1.5 text-xs ${other === locale ? "font-semibold text-fg" : "text-fg-muted hover:text-fg"}`}
            >
              {SHORT_LANGUAGE_NAME[other]}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
}

function Footer({ locale }: { locale: Locale }) {
  const copy = COPY[locale];
  return (
    <footer className="border-t border-line-muted bg-canvas-subtle">
      <div className={`${CONTAINER} grid gap-8 py-10 text-sm md:grid-cols-[1fr_auto]`}>
        <div className="space-y-2">
          <p className="flex items-center gap-2 font-semibold">
            <img src="/favicon.svg" alt="" width={20} height={20} className="size-5 rounded-[5px]" />
            VoyaVPN
          </p>
          <p className="text-fg-muted">
            <Inline text={copy.footer.tagline} />
          </p>
          <p className="text-fg-muted">
            <Inline text={copy.footer.singBox} />
          </p>
        </div>
        <ul className="flex flex-wrap gap-x-6 gap-y-2 md:justify-end">
          <li>
            <a className="text-fg-muted hover:text-fg" href={pathFor({ locale, page: "support" })}>
              {copy.nav.support}
            </a>
          </li>
          <li>
            <a className="text-fg-muted hover:text-fg" href={pathFor({ locale, page: "privacy" })}>
              {copy.nav.privacy}
            </a>
          </li>
          <li>
            <a className="text-fg-muted hover:text-fg" href={`mailto:${SUPPORT_EMAIL}`}>
              {copy.footer.contact}
            </a>
          </li>
        </ul>
        <p className="text-fg-muted md:col-span-2">© 2026 VoyaVPN</p>
      </div>
    </footer>
  );
}

/**
 * Pixel sizes of the captures in public/screens (2x). `home` is the whole
 * window; `nodes` is cropped to the list panel so its rows stay legible.
 */
const SCREENSHOT_SIZE = {
  home: { width: 2400, height: 1520 },
  nodes: { width: 1900, height: 880 },
} as const;

/** A screenshot captured from the app in the page's language, following the system theme. */
export function Screenshot({
  name,
  locale,
  alt,
  eager = false,
}: {
  name: "home" | "nodes";
  locale: Locale;
  alt: string;
  eager?: boolean;
}) {
  const base = `/screens/${name}-${locale}`;
  return (
    <picture>
      <source media="(prefers-color-scheme: dark)" srcSet={`${base}-dark.webp`} />
      <img
        src={`${base}-light.webp`}
        alt={alt}
        width={SCREENSHOT_SIZE[name].width}
        height={SCREENSHOT_SIZE[name].height}
        loading={eager ? "eager" : "lazy"}
        decoding="async"
        fetchPriority={eager ? "high" : undefined}
        className="block h-auto w-full"
      />
    </picture>
  );
}
