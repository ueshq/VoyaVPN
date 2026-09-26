import { closeSync, openSync, readSync } from "node:fs";
import { relative } from "node:path";

import { checkedCapture } from "../../lib/common.mjs";
import { walkFilesSync } from "../../lib/fs.mjs";

/**
 * What App Store validation reads out of every Mach-O in a bundle, as rules a
 * build can check before the upload does.
 *
 * App Review rejected the 2026-09 store upload (Guideline 2.5.1) because the upstream sing-box
 * release binary imports `__kCFBundleNumericVersionKey`: sing-box's
 * `with_naive_outbound` tag links Chromium's Cronet, whose
 * `base/mac/info_plist_data.mm` reads that key, and Cronet also links the
 * private `/usr/lib/libpmenergy.dylib` and `/usr/lib/libpmsample.dylib`.
 * `docs/release/macos-app-store.md` explains the from-source seed that avoids
 * them; this module is the gate that keeps them out.
 */

/** Undefined symbols Apple has named as non-public in a rejection. */
const forbiddenUndefinedSymbols = Object.freeze(["__kCFBundleNumericVersionKey"]);

/**
 * Libraries a store binary may link. Public frameworks, the app's own
 * embedded code, and the handful of `/usr/lib` libraries that are part of the
 * SDK. Anything else under `/usr/lib` is listed one by one when it turns up,
 * never by a wildcard: `/usr/lib/libpm*.dylib` is exactly what a wildcard
 * would let through.
 */
const allowedLinkedLibraryPatterns = Object.freeze([
  /^\/System\/Library\/Frameworks\//u,
  /^@(?:rpath|executable_path|loader_path)\//u,
  /^\/usr\/lib\/libSystem\.B\.dylib$/u,
  /^\/usr\/lib\/libobjc\.A\.dylib$/u,
  /^\/usr\/lib\/libc\+\+(?:abi)?(?:\.\d+)?\.dylib$/u,
  /^\/usr\/lib\/libresolv\.\d+\.dylib$/u,
  /^\/usr\/lib\/libbsm\.\d+\.dylib$/u,
  /^\/usr\/lib\/libiconv\.\d+\.dylib$/u,
  /^\/usr\/lib\/libz\.\d+\.dylib$/u,
  /^\/usr\/lib\/libsqlite3\.dylib$/u,
  /^\/usr\/lib\/swift\//u,
]);

const machOMagics = new Set([
  "feedface",
  "feedfacf",
  "cefaedfe",
  "cffaedfe",
  "cafebabe",
  "bebafeca",
]);

/** True when the first four bytes are a thin or fat Mach-O magic. */
export function isMachOHeader(bytes) {
  if (!bytes || bytes.length < 4) {
    return false;
  }
  return machOMagics.has(Buffer.from(bytes.subarray(0, 4)).toString("hex"));
}

export function isMachOFile(path) {
  const fd = openSync(path, "r");
  try {
    const header = Buffer.alloc(4);
    const read = readSync(fd, header, 0, 4, 0);
    return read === 4 && isMachOHeader(header);
  } finally {
    closeSync(fd);
  }
}

/**
 * Symbol names from `nm -u -arch all <file>`. A fat file prints a
 * `<file> (for architecture arm64):` header per slice, and an archive a
 * `<member>.o:` header per member; both end in a colon and are skipped.
 */
export function parseUndefinedSymbols(nmOutput) {
  const symbols = new Set();
  for (const raw of String(nmOutput ?? "").split("\n")) {
    const line = raw.trim();
    if (!line || line.endsWith(":")) {
      continue;
    }
    symbols.add(line.split(/\s+/u).pop());
  }
  return [...symbols];
}

/**
 * Install names from `otool -arch all -L <file>`: each tab-indented line up
 * to ` (compatibility version …)`. Header lines are not indented.
 */
export function parseLinkedLibraries(otoolOutput) {
  const libraries = new Set();
  for (const line of String(otoolOutput ?? "").split("\n")) {
    if (!/^\s/u.test(line)) {
      continue;
    }
    const match = /^\s+(.+?)\s+\(compatibility version/u.exec(line);
    if (match) {
      libraries.add(match[1]);
    }
  }
  return [...libraries];
}

export function isAllowedLinkedLibrary(installName) {
  return allowedLinkedLibraryPatterns.some((pattern) => pattern.test(installName));
}

/** One readable line per reason App Store validation would reject `name`. */
export function machOImportProblems({ name, undefinedSymbols = [], linkedLibraries = [] }) {
  const problems = [];
  for (const symbol of forbiddenUndefinedSymbols) {
    if (undefinedSymbols.includes(symbol)) {
      problems.push(`${name} imports the non-public symbol ${symbol} (App Review Guideline 2.5.1).`);
    }
  }
  for (const library of linkedLibraries) {
    if (!isAllowedLinkedLibrary(library)) {
      problems.push(`${name} links ${library}, which is not a public SDK library.`);
    }
  }
  return problems;
}

/** Every Mach-O under `root`, whatever its mode bits: executables, extensions and embedded frameworks. */
function machOBinariesIn(root) {
  return walkFilesSync(root).filter((path) => !path.includes("/_CodeSignature/") && isMachOFile(path));
}

/**
 * Scans every Mach-O under `root` with `nm` and `otool`. Names are relative
 * to `root`. Used by the Mac App Store package and the iOS pre-upload check.
 */
export function bundleImportReport(root, { captureCommand = checkedCapture } = {}) {
  const binaries = machOBinariesIn(root).map((binary) => relative(root, binary));
  const problems = binaries.flatMap((name) => {
    const binary = `${root}/${name}`;
    return machOImportProblems({
      name,
      undefinedSymbols: parseUndefinedSymbols(captureCommand("nm", ["-u", "-arch", "all", binary]).stdout),
      linkedLibraries: parseLinkedLibraries(captureCommand("otool", ["-arch", "all", "-L", binary]).stdout),
    });
  });
  return { binaries, problems };
}
