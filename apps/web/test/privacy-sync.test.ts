import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vite-plus/test";
import { renderPage } from "../src/render";
import { LOCALES } from "../src/routes";

// The App Store's VPN answers and the privacy policy must state the same facts
// (docs/release/app-store-review-notes.md says so). Every host those answers
// name has to appear in the policy in every language.
const notes = readFileSync(resolve(__dirname, "../../../docs/release/app-store-review-notes.md"), "utf8");
const vpnAnswers = notes.split("## VPN questions")[1]?.split("\n## ")[0] ?? "";
const hosts = [...new Set(vpnAnswers.match(/\b(?:[a-z0-9-]+\.)+(?:com|org|net|app|is|me)\b/g))];

// The mobile app states the same facts on its first-run screen (Guideline 5.4).
// It may name fewer hosts than the answers, because a phone has no self-hosted
// node, but never one the answers and the policy do not name.
const hostPattern = /\b(?:[a-z0-9-]+\.)+(?:com|org|net|app|is|me)\b/g;
const noticeHosts = (locale: string): string[] => {
  const path = resolve(__dirname, `../../../packages/i18n/src/locales/${locale}.json`);
  const messages = JSON.parse(readFileSync(path, "utf8")) as { mobile: { privacyNoticeRequests: string } };
  return messages.mobile.privacyNoticeRequests.match(hostPattern) ?? [];
};

describe("in-app data notice", () => {
  it.each(["en", "zh-Hans", "zh-Hant"])("names only hosts the VPN answers name, in %s", (locale) => {
    const named = noticeHosts(locale);
    expect(named).toEqual(expect.arrayContaining(["raw.githubusercontent.com", "ipwho.is", "www.google.com"]));
    for (const host of named) expect(hosts, host).toContain(host);
  });
});

describe("privacy policy", () => {
  it("finds the hosts in the review notes", () => {
    expect(hosts).toEqual(
      expect.arrayContaining(["raw.githubusercontent.com", "probe.voyavpn.app", "ipwho.is"]),
    );
  });

  it.each(LOCALES)("names every host from the VPN answers in %s", (locale) => {
    const html = renderPage(locale, "privacy", "/site.css");
    for (const host of hosts) expect(html, host).toContain(host);
  });
});
