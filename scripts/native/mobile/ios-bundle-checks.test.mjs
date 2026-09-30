import { describe, expect, it } from "vitest";

import { repoRootFromScript } from "../../lib/common.mjs";
import {
  iosBundleProblems,
  parsePlist,
  parseStrings,
  purposeStringProblems,
  tunnelIdentityProblems,
} from "./ios-bundle-checks.mjs";

const plist = (body) => `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
${body}
</plist>`;

describe("property list and strings parsing", () => {
  it("reads the value kinds Info.plist and entitlements use", () => {
    expect(
      parsePlist(
        plist(`<dict>
  <!-- a comment with <angle> brackets -->
  <key>Name</key>
  <string>A &amp; B &lt;c&gt;</string>
  <key>Flag</key>
  <true/>
  <key>Off</key>
  <false/>
  <key>Count</key>
  <integer>3</integer>
  <key>List</key>
  <array>
    <string>one</string>
    <string>two</string>
  </array>
  <key>Nested</key>
  <dict>
    <key>Empty</key>
    <array/>
  </dict>
</dict>`),
      ),
    ).toEqual({ Name: "A & B <c>", Flag: true, Off: false, Count: 3, List: ["one", "two"], Nested: { Empty: [] } });
    expect(() => parsePlist("bplist00")).toThrow(/Not an XML property list/u);
  });

  it("reads .strings entries, including escaped quotes", () => {
    expect(parseStrings('"A" = "one";\n/* note */\n"B" = "say \\"hi\\"";\n')).toEqual({ A: "one", B: 'say "hi"' });
  });
});

describe("purpose strings", () => {
  const infoPlist = { NSCameraUsageDescription: "Scan a QR code.", CFBundleName: "VoyaVPN" };

  it("accepts a purpose string translated everywhere", () => {
    const localized = { en: { NSCameraUsageDescription: "Scan." }, "zh-Hans": { NSCameraUsageDescription: "扫描。" } };
    expect(purposeStringProblems({ infoPlist, localized })).toEqual([]);
  });

  it("reports an empty base string, a missing translation and a stray one", () => {
    const problems = purposeStringProblems({
      infoPlist: { ...infoPlist, NSLocalNetworkUsageDescription: " " },
      localized: {
        en: { NSCameraUsageDescription: "Scan.", NSLocalNetworkUsageDescription: "Tests latency." },
        "zh-Hans": { NSCameraUsageDescription: "扫描。", NSLocationWhenInUseUsageDescription: "" },
      },
    });
    expect(problems).toEqual([
      "Info.plist NSLocalNetworkUsageDescription is empty.",
      "zh-Hans.lproj/InfoPlist.strings has no NSLocalNetworkUsageDescription.",
      "zh-Hans.lproj/InfoPlist.strings translates NSLocationWhenInUseUsageDescription, which Info.plist does not declare.",
    ]);
  });
});

describe("app and PacketTunnel identity", () => {
  const group = "group.app.voyavpn.mobile";
  const entitlements = {
    "com.apple.developer.networking.networkextension": ["packet-tunnel-provider"],
    "com.apple.security.application-groups": [group],
  };
  const good = {
    appPlist: { VoyaAppGroupIdentifier: group },
    appexPlist: {
      VoyaAppGroupIdentifier: group,
      NSExtension: { NSExtensionPointIdentifier: "com.apple.networkextension.packet-tunnel" },
    },
    appEntitlements: entitlements,
    appexEntitlements: entitlements,
  };

  it("accepts matching identifiers", () => {
    expect(tunnelIdentityProblems(good)).toEqual([]);
  });

  // The desktop's group here would let the phone build read another app's container.
  it("rejects a PacketTunnel that names another App Group", () => {
    const problems = tunnelIdentityProblems({
      ...good,
      appexPlist: { ...good.appexPlist, VoyaAppGroupIdentifier: "group.app.voyavpn.desktop" },
      appexEntitlements: { ...entitlements, "com.apple.security.application-groups": ["group.app.voyavpn.desktop"] },
    });
    expect(problems).toHaveLength(2);
    expect(problems[0]).toMatch(/group\.app\.voyavpn\.desktop/u);
    expect(problems[1]).toBe(`The PacketTunnel entitlements must list exactly the App Group ${group}.`);
  });

  it("rejects an extra capability and a wrong extension point", () => {
    const problems = tunnelIdentityProblems({
      ...good,
      appEntitlements: {
        ...entitlements,
        "com.apple.developer.networking.networkextension": ["packet-tunnel-provider", "app-proxy-provider"],
      },
      appexPlist: { VoyaAppGroupIdentifier: group, NSExtension: {}, RCTNewArchEnabled: true },
    });
    expect(problems).toEqual([
      "The app entitlements must claim exactly packet-tunnel-provider.",
      "The PacketTunnel's NSExtensionPointIdentifier must be com.apple.networkextension.packet-tunnel.",
    ]);
  });
});

describe("the checked-in iOS bundle inputs", () => {
  it("pass every check", () => {
    expect(iosBundleProblems(repoRootFromScript(import.meta.url))).toEqual([]);
  });
});
