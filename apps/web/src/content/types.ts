import type { Platform } from "../site";

/**
 * Inline text. Backticks mark code (`vless://`, host names); nothing else is
 * parsed, so copy stays plain strings.
 */
type Text = string;

/** A paragraph, or a bulleted list. */
export type Block = Text | { list: Text[] };

type Section = { heading: Text; body: Block[] };

type Feature = { title: Text; body: Text };

export type FeatureId = "rules" | "vpn" | "selfHost" | "activity" | "exit";

/**
 * Every locale implements this whole shape, so TypeScript keeps the three
 * languages aligned without a key checker. Site copy is kept out of
 * packages/i18n: the desktop bundles every locale JSON there, and none of this
 * text belongs in the app.
 */
export interface SiteCopy {
  languageName: string;
  nav: {
    home: string;
    support: string;
    privacy: string;
    skip: string;
    languages: string;
  };
  footer: {
    tagline: Text;
    singBox: Text;
    contact: string;
  };
  home: {
    title: string;
    description: string;
    headline: string;
    sub: Text;
    download: string;
    features: string;
    screenshotAlt: string;
    privacy: {
      headline: string;
      body: Text;
      keptTitle: string;
      kept: Text[];
      neverTitle: string;
      never: Text[];
      link: string;
    };
    featuresHeadline: string;
    nodes: Feature & { alt: string };
    items: Record<FeatureId, Feature>;
    protocolsHeadline: string;
    protocolsBody: Text;
    downloadHeadline: string;
    downloadBody: Text;
    comingSoon: string;
    getIt: string;
    platforms: Record<Platform, string>;
  };
  support: {
    title: string;
    description: string;
    headline: string;
    intro: Text;
    contactHeadline: string;
    contactBody: Text;
    faqHeadline: string;
    faq: { question: string; answer: Block[] }[];
  };
  privacy: {
    title: string;
    description: string;
    headline: string;
    /** Contains `{date}`, replaced by the effective date. */
    effective: string;
    intro: Text;
    sections: Section[];
    contactHeading: string;
    /** Contains `{email}`, replaced by the support address as a link. */
    contactBody: Text;
  };
  notFound: {
    title: string;
    headline: string;
    body: Text;
    back: string;
  };
}
