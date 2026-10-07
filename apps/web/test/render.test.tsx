import { describe, expect, it } from "vite-plus/test";
import { COPY } from "../src/content";
import { renderSite } from "../src/render";
import { LOCALES, ROUTES, fileFor, pathFor, urlFor } from "../src/routes";

const CSS = "/assets/site-test.css";
const files = new Map(renderSite(CSS).map(({ file, contents }) => [file, contents]));

function page(file: string): string {
  const html = files.get(file);
  if (html === undefined) throw new Error(`${file} was not rendered`);
  return html;
}

describe("routes", () => {
  it("serves the App Store URLs at the root without a trailing slash", () => {
    expect(urlFor({ locale: "en", page: "support" })).toBe("https://voyavpn.wangc.ai/support");
    expect(urlFor({ locale: "en", page: "privacy" })).toBe("https://voyavpn.wangc.ai/privacy");
    expect(fileFor({ locale: "en", page: "support" })).toBe("support.html");
    expect(fileFor({ locale: "en", page: "privacy" })).toBe("privacy.html");
  });

  it("puts each translation under its own prefix", () => {
    expect(pathFor({ locale: "zh-hans", page: "home" })).toBe("/zh-hans/");
    expect(fileFor({ locale: "zh-hans", page: "home" })).toBe("zh-hans/index.html");
    expect(pathFor({ locale: "zh-hant", page: "privacy" })).toBe("/zh-hant/privacy");
    expect(fileFor({ locale: "zh-hant", page: "privacy" })).toBe("zh-hant/privacy.html");
  });
});

describe("renderSite", () => {
  it("renders every page, a 404 per locale and a sitemap", () => {
    expect([...files.keys()].sort()).toEqual(
      [...ROUTES.map(fileFor), "404.html", "zh-hans/404.html", "zh-hant/404.html", "sitemap.xml"].sort(),
    );
    const sitemap = page("sitemap.xml");
    for (const route of ROUTES) expect(sitemap).toContain(`<loc>${urlFor(route)}</loc>`);
  });

  it.each(ROUTES)("$locale $page is a complete document with canonical and alternates", (route) => {
    const html = page(fileFor(route));
    const lang = { en: "en", "zh-hans": "zh-Hans", "zh-hant": "zh-Hant" }[route.locale];
    expect(html.startsWith(`<!doctype html><html lang="${lang}">`)).toBe(true);
    expect(html).toContain(`<title>${COPY[route.locale][route.page].title}</title>`);
    expect(html).toContain(`<link rel="canonical" href="${urlFor(route)}"/>`);
    for (const locale of LOCALES) {
      expect(html).toContain(`href="${urlFor({ locale, page: route.page })}"`);
    }
    expect(html).toContain(`hrefLang="x-default" href="${urlFor({ ...route, locale: "en" })}"`);
    expect(html).toContain(`<link rel="stylesheet" href="${CSS}"/>`);
  });

  it("ships no script and no inline style, as the CSP allows neither", () => {
    for (const [file, html] of files) {
      if (!file.endsWith(".html")) continue;
      expect(html, file).not.toMatch(/<script|\sstyle="|\son[a-z]+=/i);
    }
  });

  it("links only to pages, anchors and assets that exist", () => {
    const pagePaths = new Set(ROUTES.map(pathFor));
    for (const [file, html] of files) {
      if (!file.endsWith(".html")) continue;
      for (const [, href] of html.matchAll(/<a [^>]*href="([^"]+)"/g)) {
        if (href.startsWith("mailto:")) continue;
        if (href.startsWith("#")) {
          expect(html, `${file} -> ${href}`).toContain(`id="${href.slice(1)}"`);
          continue;
        }
        expect(pagePaths.has(href), `${file} -> ${href}`).toBe(true);
      }
    }
  });

  it("marks noindex on 404 pages and gives them no canonical", () => {
    const html = page("zh-hans/404.html");
    expect(html).toContain('<meta name="robots" content="noindex"/>');
    expect(html).not.toContain('rel="canonical"');
    expect(html).toContain(COPY["zh-hans"].notFound.headline);
  });

  it("keeps the em dash out of visible copy", () => {
    for (const [file, html] of files) expect(html, file).not.toMatch(/[—–]/);
  });
});
