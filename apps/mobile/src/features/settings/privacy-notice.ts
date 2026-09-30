/**
 * The data notice shown before the app can be used.
 *
 * App Store Guideline 5.4 requires a VPN app to declare, on a screen and before
 * any use, what user data it collects and how it is used. The statements are
 * the same facts as the VPN answers in docs/release/app-store-review-notes.md
 * and the privacy policy page; change them together.
 *
 * Raise the version when the wording changes in a way users should see again:
 * an acceptance of an older version no longer counts.
 */
export const PRIVACY_NOTICE_VERSION = 1;
export const PRIVACY_POLICY_URL = "https://voyavpn.wangc.ai/privacy";
export const SUPPORT_URL = "https://voyavpn.wangc.ai/support";

export function isPrivacyNoticeAccepted(acceptedVersion: number | null): boolean {
  return acceptedVersion !== null && acceptedVersion >= PRIVACY_NOTICE_VERSION;
}
