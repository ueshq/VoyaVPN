import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { renderPage } from "../src/render";
import { LOCALES } from "../src/routes";

// The App Store's VPN answers and the privacy policy must state the same facts
// (docs/release/app-store-review-notes.md says so). Every host those answers
// name has to appear in the policy in every language.
const notes = readFileSync(resolve(__dirname, "../../../docs/release/app-store-review-notes.md"), "utf8");
const vpnAnswers = notes.split("## VPN questions")[1]?.split("\n## ")[0] ?? "";
const hosts = [...new Set(vpnAnswers.match(/\b(?:[a-z0-9-]+\.)+(?:com|org|net|app|is|me)\b/g))];

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
