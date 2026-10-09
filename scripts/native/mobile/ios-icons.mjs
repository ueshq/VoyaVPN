import { readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

import { isCliEntrypoint, repoRootFromScript, requireDarwin, run } from "../../lib/common.mjs";

/**
 * The iOS app icon set, and the check that keeps it uploadable.
 *
 * App Store Connect rejects an app whose 1024px icon has an alpha channel
 * (ITMS-90717). Every generator we have writes RGBA, so the set is flattened
 * onto the icon's own background colour after it is generated:
 *
 *   apps/desktop/node_modules/.bin/tauri icon apps/desktop/src-tauri/app-icon.svg \
 *     --ios-color "#1A58F2" -o <scratch>
 *   cp <scratch>/ios/*.png apps/mobile/ios/VoyaVPN/Images.xcassets/AppIcon.appiconset/
 *   vp run native mobile ios icons          # flatten, then check
 *   vp run native mobile ios icons --check  # check only; no Xcode needed
 */

/** The `fill` of the rounded rect in `apps/desktop/src-tauri/app-icon.svg`. */
const IOS_ICON_BACKGROUND = "#1A58F2";
const appIconSet = "apps/mobile/ios/VoyaVPN/Images.xcassets/AppIcon.appiconset";

const pngSignature = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/** Size and transparency of a PNG, from its chunk headers alone. */
export function pngInfo(bytes) {
  if (bytes.length < 33 || !bytes.subarray(0, 8).equals(pngSignature) || bytes.toString("latin1", 12, 16) !== "IHDR") {
    return null;
  }
  const colorType = bytes[25];
  let hasTransparencyChunk = false;
  for (let offset = 8; offset + 8 <= bytes.length;) {
    const length = bytes.readUInt32BE(offset);
    const type = bytes.toString("latin1", offset + 4, offset + 8);
    if (type === "tRNS") hasTransparencyChunk = true;
    if (type === "IDAT" || type === "IEND") break;
    offset += 12 + length;
  }
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
    // Colour types 4 and 6 carry an alpha channel; tRNS adds one to the rest.
    hasAlpha: colorType === 4 || colorType === 6 || hasTransparencyChunk,
  };
}

function expectedPixels(image) {
  const points = Number.parseFloat(String(image.size).split("x")[0]);
  const scale = Number.parseFloat(String(image.scale));
  return Math.round(points * scale);
}

/**
 * One readable line per reason Xcode or App Store Connect would refuse the
 * icon set. `contents` is the parsed `Contents.json`; `files` maps each file
 * name in the folder to its bytes.
 */
export function iconProblems({ contents, files }) {
  const problems = [];
  const images = contents?.images ?? [];
  const referenced = new Set();
  for (const image of images) {
    const label = `${image.idiom} ${image.size}@${image.scale}`;
    if (!image.filename) {
      problems.push(`${label} names no file.`);
      continue;
    }
    referenced.add(image.filename);
    const bytes = files.get(image.filename);
    if (!bytes) {
      problems.push(`${image.filename} (${label}) is missing.`);
      continue;
    }
    const info = pngInfo(bytes);
    if (!info) {
      problems.push(`${image.filename} is not a PNG.`);
      continue;
    }
    const pixels = expectedPixels(image);
    if (info.width !== pixels || info.height !== pixels) {
      problems.push(`${image.filename} is ${info.width}x${info.height}, expected ${pixels}x${pixels} for ${label}.`);
    }
    if (info.hasAlpha) {
      problems.push(`${image.filename} has an alpha channel (ITMS-90717); run vp run native mobile ios icons.`);
    }
  }
  const marketing = images.filter((image) => image.idiom === "ios-marketing" && expectedPixels(image) === 1024);
  if (marketing.length !== 1) {
    problems.push(`Expected exactly one 1024px ios-marketing icon, found ${marketing.length}.`);
  }
  for (const name of files.keys()) {
    if (name.endsWith(".png") && !referenced.has(name)) {
      problems.push(`${name} is not referenced by Contents.json.`);
    }
  }
  return problems;
}

/** Reads the app's icon set from a checkout into the shape `iconProblems` takes. */
export function readIconSet(repoRoot) {
  const directory = resolve(repoRoot, appIconSet);
  const names = readdirSync(directory).filter((name) => name !== "Contents.json");
  return {
    directory,
    contents: JSON.parse(readFileSync(resolve(directory, "Contents.json"), "utf8")),
    files: new Map(names.map((name) => [name, readFileSync(resolve(directory, name))])),
  };
}

function main(argv) {
  const repoRoot = repoRootFromScript(import.meta.url);
  if (!argv.includes("--check")) {
    requireDarwin("Flattening the icon set needs CoreGraphics; run it on macOS.");
    const { directory, files } = readIconSet(repoRoot);
    const pngs = [...files.keys()].filter((name) => name.endsWith(".png")).map((name) => resolve(directory, name));
    run("swift", [resolve(repoRoot, "scripts/native/mobile/flatten-png.swift"), IOS_ICON_BACKGROUND, ...pngs], {
      cwd: repoRoot,
    });
  }
  const problems = iconProblems(readIconSet(repoRoot));
  if (problems.length > 0) {
    throw new Error(`iOS app icons:\n${problems.map((line) => `  - ${line}`).join("\n")}`);
  }
  console.log("✓ iOS app icons: every size present, opaque, one 1024px marketing icon");
}

if (isCliEntrypoint(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
