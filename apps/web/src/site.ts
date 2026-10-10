/** The one origin the site is served from; canonical links are built on it. */
export const SITE_ORIGIN = "https://voyavpn.wangc.ai";

export const SUPPORT_EMAIL = "support@wangc.ai";

/** Change it whenever the privacy policy's substance changes. */
export const PRIVACY_EFFECTIVE_DATE = "2026-09-27";

export type Platform = "macos" | "ios" | "android" | "windows" | "linux";

/** A build of one platform that needs its own link, named in `SiteCopy.downloadVariants`. */
export type DownloadVariant = "appleSilicon" | "intel";

export type DownloadLink = { url: string; variant?: DownloadVariant };

/** The GitHub release the direct-download links point at; bump it with each release. */
const RELEASE_VERSION = "0.1.0";
const releaseAsset = (name: string) => `https://github.com/ueshq/VoyaVPN/releases/download/v${RELEASE_VERSION}/${name}`;

/**
 * Where each platform's build can be downloaded. `null` renders as
 * "coming soon"; fill the links in when a store listing or release goes live.
 * A link without a variant reads "Get it".
 */
export const DOWNLOADS: Record<Platform, readonly DownloadLink[] | null> = {
  macos: [
    { url: releaseAsset(`VoyaVPN_${RELEASE_VERSION}_aarch64.dmg`), variant: "appleSilicon" },
    { url: releaseAsset(`VoyaVPN_${RELEASE_VERSION}_x64.dmg`), variant: "intel" },
  ],
  ios: null,
  android: null,
  windows: null,
  linux: null,
};
