import { renderToStaticMarkup } from "react-dom/server";
import { COPY } from "./content";
import { Document } from "./layout";
import { HomePage } from "./pages/home";
import { NotFoundPage } from "./pages/not-found";
import { PrivacyPage } from "./pages/privacy";
import { SupportPage } from "./pages/support";
import { LOCALES, type Locale, type Page, ROUTES, fileFor, notFoundFileFor, urlFor } from "./routes";

type OutputFile = { file: string; contents: string };

const PAGE_BODY: Record<Page, (props: { locale: Locale }) => React.ReactNode> = {
  home: HomePage,
  support: SupportPage,
  privacy: PrivacyPage,
};

function html(node: React.ReactElement): string {
  return `<!doctype html>${renderToStaticMarkup(node)}`;
}

export function renderPage(locale: Locale, page: Page, css: string): string {
  const copy = COPY[locale][page];
  const Body = PAGE_BODY[page];
  return html(
    <Document locale={locale} page={page} title={copy.title} description={copy.description} css={css}>
      <Body locale={locale} />
    </Document>,
  );
}

function renderNotFound(locale: Locale, css: string): string {
  const copy = COPY[locale].notFound;
  return html(
    <Document locale={locale} page={null} title={copy.title} description={copy.body} css={css}>
      <NotFoundPage locale={locale} />
    </Document>,
  );
}

function sitemap(): string {
  const entries = ROUTES.map((route) => `  <url><loc>${urlFor(route)}</loc></url>`).join("\n");
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${entries}\n</urlset>\n`;
}

/**
 * Every generated file, with `css` as the public path of the built stylesheet.
 * @public Called by scripts/build.mjs through Vite's module runner.
 */
export function renderSite(css: string): OutputFile[] {
  return [
    ...ROUTES.map(({ locale, page }) => ({
      file: fileFor({ locale, page }),
      contents: renderPage(locale, page, css),
    })),
    ...LOCALES.map((locale) => ({
      file: notFoundFileFor(locale),
      contents: renderNotFound(locale, css),
    })),
    { file: "sitemap.xml", contents: sitemap() },
  ];
}
