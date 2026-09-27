/** The one origin the site is served from; canonical links are built on it. */
export const SITE_ORIGIN = "https://voyavpn.wangc.ai";

export const SUPPORT_EMAIL = "support@wangc.ai";

/** Change it whenever the privacy policy's substance changes. */
export const PRIVACY_EFFECTIVE_DATE = "2026-09-27";

export type Platform = "macos" | "ios" | "android" | "windows" | "linux";

/**
 * Where each platform's build can be downloaded. `null` renders as
 * "coming soon"; fill a URL in when a store listing or release goes live.
 */
export const DOWNLOADS: Record<Platform, string | null> = {
  macos: null,
  ios: null,
  android: null,
  windows: null,
  linux: null,
};
