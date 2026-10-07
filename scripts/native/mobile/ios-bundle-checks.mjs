import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { isCliEntrypoint, repoRootFromScript } from "../../lib/common.mjs";
import { iconProblems, readIconSet } from "./ios-icons.mjs";
import { checkMobileLegalAssets } from "./legal-assets.mjs";

/**
 * What App Store Connect and App Review read out of the iOS app's checked-in
 * bundle inputs, as rules that run on any OS: the icon set, the purpose
 * strings, and the tunnel extension's identity. `pnpm check:mobile:ios:assets`
 * runs them, and `pnpm build:ios:appstore` runs them before it archives.
 */

/**
 * The oldest iOS the app and its tunnel extension run on.
 *
 * `ios-project.rb` writes it into the Xcode project as `DEPLOYMENT_TARGET`
 * (a test holds the two together), the store lane checks the built bundles
 * against it, and the smoke lane compiles the probe-core host for it — built
 * for anything newer, a call that does not exist on this version would
 * compile there and fail on a user's phone.
 */
export const IOS_DEPLOYMENT_TARGET = "15.1";

const locales = ["en", "zh-Hans", "zh-Hant"];
const tunnelExtensionPoint = "com.apple.networkextension.packet-tunnel";
const networkExtensionKey = "com.apple.developer.networking.networkextension";
const appGroupsKey = "com.apple.security.application-groups";

function unescapeXml(text) {
  return text
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&quot;", '"')
    .replaceAll("&apos;", "'")
    .replaceAll("&amp;", "&");
}

/**
 * Parses an XML property list into plain values. It covers what Info.plist
 * and entitlements files use (dict, array, string, integer, true, false),
 * which keeps these checks free of `plutil` and so runnable on Linux CI.
 */
export function parsePlist(xml) {
  const tokens =
    String(xml)
      .replace(/<\?xml[\s\S]*?\?>|<!DOCTYPE[\s\S]*?>|<!--[\s\S]*?-->/gu, "")
      .match(/<[^>]+>|[^<]+/gu) ?? [];
  let index = 0;
  const next = () => tokens[index++];
  const skipSpace = () => {
    while (index < tokens.length && !tokens[index].startsWith("<") && tokens[index].trim() === "") index += 1;
  };
  const text = (closing) => {
    let value = "";
    while (index < tokens.length && tokens[index] !== closing) value += next();
    next();
    return unescapeXml(value);
  };
  const value = () => {
    skipSpace();
    const tag = next();
    switch (tag) {
      case "<true/>":
        return true;
      case "<false/>":
        return false;
      case "<string/>":
        return "";
      case "<array/>":
        return [];
      case "<dict/>":
        return {};
      case "<string>":
        return text("</string>");
      case "<integer>":
        return Number.parseInt(text("</integer>"), 10);
      case "<array>": {
        const items = [];
        for (skipSpace(); tokens[index] !== "</array>"; skipSpace()) items.push(value());
        next();
        return items;
      }
      case "<dict>": {
        const entries = {};
        for (skipSpace(); tokens[index] !== "</dict>"; skipSpace()) {
          if (next() !== "<key>") throw new Error("Malformed property list: expected <key>.");
          const key = text("</key>");
          entries[key] = value();
        }
        next();
        return entries;
      }
      default:
        throw new Error(`Unsupported property list element ${tag}.`);
    }
  };
  skipSpace();
  if (!/^<plist\b/u.test(next() ?? "")) throw new Error("Not an XML property list.");
  return value();
}

/** Parses a `.strings` file of `"key" = "value";` lines. */
export function parseStrings(text) {
  const entries = {};
  for (const match of String(text).matchAll(/"((?:[^"\\]|\\.)*)"\s*=\s*"((?:[^"\\]|\\.)*)"\s*;/gu)) {
    entries[match[1]] = match[2].replace(/\\(.)/gu, "$1");
  }
  return entries;
}

/**
 * App Review rejects an empty or missing purpose string, and shows the base
 * one to a user whose language has no translation. `localized` maps each
 * locale to its parsed `InfoPlist.strings`.
 */
export function purposeStringProblems({ infoPlist, localized }) {
  const problems = [];
  const keys = Object.keys(infoPlist).filter((key) => /^NS\w+UsageDescription$/u.test(key));
  for (const key of keys) {
    if (String(infoPlist[key]).trim() === "") problems.push(`Info.plist ${key} is empty.`);
    for (const [locale, strings] of Object.entries(localized)) {
      if (String(strings[key] ?? "").trim() === "") {
        problems.push(`${locale}.lproj/InfoPlist.strings has no ${key}.`);
      }
    }
  }
  for (const [locale, strings] of Object.entries(localized)) {
    for (const key of Object.keys(strings)) {
      if (!keys.includes(key)) {
        problems.push(`${locale}.lproj/InfoPlist.strings translates ${key}, which Info.plist does not declare.`);
      }
    }
  }
  return problems;
}

/**
 * The app and its PacketTunnel must name the same App Group in four places
 * (two Info.plists, two entitlements files) and claim exactly the packet
 * tunnel provider capability; see docs/release/mobile-ios-signing.md.
 */
export function tunnelIdentityProblems({ appPlist, appexPlist, appEntitlements, appexEntitlements }) {
  const problems = [];
  const group = appPlist.VoyaAppGroupIdentifier;
  if (!group) problems.push("The app's Info.plist declares no VoyaAppGroupIdentifier.");
  if (appexPlist.VoyaAppGroupIdentifier !== group) {
    problems.push(
      `The PacketTunnel declares App Group ${JSON.stringify(appexPlist.VoyaAppGroupIdentifier)}, the app ${JSON.stringify(group)}.`,
    );
  }
  for (const [name, entitlements] of [
    ["app", appEntitlements],
    ["PacketTunnel", appexEntitlements],
  ]) {
    if (JSON.stringify(entitlements[appGroupsKey]) !== JSON.stringify([group])) {
      problems.push(`The ${name} entitlements must list exactly the App Group ${group}.`);
    }
    if (JSON.stringify(entitlements[networkExtensionKey]) !== JSON.stringify(["packet-tunnel-provider"])) {
      problems.push(`The ${name} entitlements must claim exactly packet-tunnel-provider.`);
    }
  }
  if (appexPlist.NSExtension?.NSExtensionPointIdentifier !== tunnelExtensionPoint) {
    problems.push(`The PacketTunnel's NSExtensionPointIdentifier must be ${tunnelExtensionPoint}.`);
  }
  // `RCTNewArchEnabled` in the extension's Info.plist is not a problem, and is
  // not checked: React Native's `pod install` writes it into every Info.plist
  // in the project, so removing it only lasts until the next install.
  return problems;
}

/** Every checked-in bundle input, read from a checkout. */
export function iosBundleProblems(repoRoot) {
  const ios = resolve(repoRoot, "apps/mobile/ios");
  const plist = (path) => parsePlist(readFileSync(resolve(ios, path), "utf8"));
  const appPlist = plist("VoyaVPN/Info.plist");
  return [
    ...iconProblems(readIconSet(repoRoot)),
    ...purposeStringProblems({
      infoPlist: appPlist,
      localized: Object.fromEntries(
        locales.map((locale) => [
          locale,
          parseStrings(readFileSync(resolve(ios, `VoyaVPN/${locale}.lproj/InfoPlist.strings`), "utf8")),
        ]),
      ),
    }),
    ...tunnelIdentityProblems({
      appPlist,
      appexPlist: plist("PacketTunnel/Info.plist"),
      appEntitlements: plist("VoyaVPN/VoyaVPN.entitlements"),
      appexEntitlements: plist("PacketTunnel/PacketTunnel.entitlements"),
    }),
  ];
}

/** Throws with every problem, or prints one line per passed group. */
export function checkIosBundleInputs(repoRoot) {
  const problems = iosBundleProblems(repoRoot);
  if (problems.length > 0) {
    throw new Error(`iOS bundle inputs:\n${problems.map((line) => `  - ${line}`).join("\n")}`);
  }
  checkMobileLegalAssets();
  console.log("✓ iOS app icons are complete and opaque");
  console.log("✓ Purpose strings are present in every language");
  console.log("✓ App and PacketTunnel agree on the App Group and claim only packet-tunnel-provider");
  console.log("✓ Bundled legal notices are current");
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    checkIosBundleInputs(repoRootFromScript(import.meta.url));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
