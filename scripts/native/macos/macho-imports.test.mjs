import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

import {
  bundleImportReport,
  isAllowedLinkedLibrary,
  isMachOHeader,
  machOImportProblems,
  parseLinkedLibraries,
  parseUndefinedSymbols,
} from "./macho-imports.mjs";

// Excerpts of `nm -u -arch all` and `otool -arch all -L` on the upstream
// sing-box 1.13.14 darwin-arm64 release binary, the one App Review rejected.
const upstreamSeedNm = `_CFAbsoluteTimeGetCurrent
_CFArrayAppendValue
_CFBundleGetMainBundle
__kCFBundleNumericVersionKey
_kCFBundleVersionKey
`;
const upstreamSeedOtool = `apps/desktop/src-tauri/resources/core-seeds/sing_box/sing-box:
\t/usr/lib/libresolv.9.dylib (compatibility version 1.0.0, current version 1.0.0)
\t/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation (compatibility version 150.0.0, current version 5026.5.4)
\t/usr/lib/libbsm.0.dylib (compatibility version 1.0.0, current version 1.0.0)
\t/usr/lib/libpmenergy.dylib (compatibility version 1.0.0, current version 2.0.0)
\t/usr/lib/libpmsample.dylib (compatibility version 1.0.0, current version 2.0.0)
\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1356.0.0)
`;

describe("Mach-O public API gate", () => {
  it("reports the symbol and both private libraries App Review found in the upstream seed", () => {
    expect(
      machOImportProblems({
        name: "Resources/core-seeds/sing_box/sing-box",
        undefinedSymbols: parseUndefinedSymbols(upstreamSeedNm),
        linkedLibraries: parseLinkedLibraries(upstreamSeedOtool),
      }),
    ).toEqual([
      "Resources/core-seeds/sing_box/sing-box imports the non-public symbol __kCFBundleNumericVersionKey (App Review Guideline 2.5.1).",
      "Resources/core-seeds/sing_box/sing-box links /usr/lib/libpmenergy.dylib, which is not a public SDK library.",
      "Resources/core-seeds/sing_box/sing-box links /usr/lib/libpmsample.dylib, which is not a public SDK library.",
    ]);
  });

  it("accepts a binary that links only public frameworks and SDK libraries", () => {
    const otool = `target/release/voyavpn:
\t/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit (compatibility version 45.0.0, current version 2685.60.104)
\t/usr/lib/libobjc.A.dylib (compatibility version 1.0.0, current version 228.0.0)
\t/usr/lib/libiconv.2.dylib (compatibility version 7.0.0, current version 7.0.0)
\t/usr/lib/libc++.1.dylib (compatibility version 1.0.0, current version 1900.180.0)
\t@rpath/Libbox.framework/Versions/A/Libbox (compatibility version 0.0.0, current version 0.0.0)
\t/usr/lib/swift/libswiftCore.dylib (compatibility version 1.0.0, current version 6.2.0)
`;
    expect(
      machOImportProblems({
        name: "MacOS/voyavpn",
        undefinedSymbols: parseUndefinedSymbols("_kCFBundleVersionKey\n_objc_msgSend\n"),
        linkedLibraries: parseLinkedLibraries(otool),
      }),
    ).toEqual([]);
  });

  it("skips per-architecture and per-member headers", () => {
    const fat = `/b/sing-box (for architecture arm64):
_CFRelease
/b/sing-box (for architecture x86_64):
_CFRelease
__kCFBundleNumericVersionKey
`;
    expect(parseUndefinedSymbols(fat)).toEqual(["_CFRelease", "__kCFBundleNumericVersionKey"]);

    const archive = `Libbox(info_plist_data.o):
                 U __kCFBundleNumericVersionKey
`;
    expect(parseUndefinedSymbols(archive)).toEqual(["__kCFBundleNumericVersionKey"]);

    const fatOtool = `/b/x (architecture arm64):
\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)
/b/x (architecture x86_64):
\t/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)
`;
    expect(parseLinkedLibraries(fatOtool)).toEqual(["/usr/lib/libSystem.B.dylib"]);
  });

  it("rejects private frameworks and anything outside the system", () => {
    expect(isAllowedLinkedLibrary("/System/Library/PrivateFrameworks/X.framework/X")).toBe(false);
    expect(isAllowedLinkedLibrary("/opt/homebrew/lib/libfoo.dylib")).toBe(false);
    expect(isAllowedLinkedLibrary("/usr/local/lib/libfoo.dylib")).toBe(false);
    expect(isAllowedLinkedLibrary("/usr/lib/libpmenergy.dylib")).toBe(false);
    expect(isAllowedLinkedLibrary("/System/Library/Frameworks/Network.framework/Versions/A/Network")).toBe(true);
  });

  it("recognizes thin and fat Mach-O headers only", () => {
    expect(isMachOHeader(Buffer.from([0xcf, 0xfa, 0xed, 0xfe, 0x0c]))).toBe(true);
    expect(isMachOHeader(Buffer.from([0xca, 0xfe, 0xba, 0xbe]))).toBe(true);
    expect(isMachOHeader(Buffer.from("#!/bin/sh\n"))).toBe(false);
    expect(isMachOHeader(Buffer.from([0xcf]))).toBe(false);
  });

  it("scans every Mach-O in a bundle and names each relative to it", () => {
    const root = mkdtempSync(join(tmpdir(), "voyavpn-macho-"));
    try {
      const thin = Buffer.from([0xcf, 0xfa, 0xed, 0xfe, 0, 0, 0, 0]);
      mkdirSync(join(root, "MacOS"), { recursive: true });
      mkdirSync(join(root, "Resources", "core-seeds", "sing_box"), { recursive: true });
      mkdirSync(join(root, "_CodeSignature"), { recursive: true });
      writeFileSync(join(root, "MacOS", "voyavpn"), thin);
      writeFileSync(join(root, "Resources", "core-seeds", "sing_box", "sing-box"), thin);
      writeFileSync(join(root, "Resources", "core-seeds", "sing_box", "sing-box.seed.json"), "{}");
      writeFileSync(join(root, "_CodeSignature", "CodeResources"), thin);

      const captureCommand = (program, args) => {
        const seed = args.at(-1).endsWith("sing-box");
        if (program === "nm") {
          return { stdout: seed ? upstreamSeedNm : "_objc_msgSend\n" };
        }
        return { stdout: seed ? upstreamSeedOtool : "" };
      };
      const report = bundleImportReport(root, { captureCommand });
      expect(report.binaries.sort()).toEqual(["MacOS/voyavpn", "Resources/core-seeds/sing_box/sing-box"]);
      expect(report.problems).toHaveLength(3);
      expect(report.problems.every((line) => line.startsWith("Resources/core-seeds/sing_box/sing-box "))).toBe(true);
    } finally {
      rmSync(root, { force: true, recursive: true });
    }
  });
});
