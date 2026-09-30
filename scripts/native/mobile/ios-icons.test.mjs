import { describe, expect, it } from "vitest";

import { repoRootFromScript } from "../../lib/common.mjs";
import { iconProblems, pngInfo, readIconSet } from "./ios-icons.mjs";

/** A PNG header with the given size and colour type; enough for `pngInfo`. */
function png({ size, colorType = 2, transparencyChunk = false }) {
  const chunk = (type, data) => {
    const header = Buffer.alloc(8);
    header.writeUInt32BE(data.length, 0);
    header.write(type, 4, "latin1");
    return Buffer.concat([header, data, Buffer.alloc(4)]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = colorType;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    ...(transparencyChunk ? [chunk("tRNS", Buffer.alloc(2))] : []),
    chunk("IDAT", Buffer.alloc(1)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const contents = {
  images: [
    { idiom: "iphone", size: "60x60", scale: "3x", filename: "icon-180.png" },
    { idiom: "ipad", size: "83.5x83.5", scale: "2x", filename: "icon-167.png" },
    { idiom: "ios-marketing", size: "1024x1024", scale: "1x", filename: "icon-1024.png" },
  ],
};
const goodFiles = () =>
  new Map([
    ["icon-180.png", png({ size: 180 })],
    ["icon-167.png", png({ size: 167 })],
    ["icon-1024.png", png({ size: 1024 })],
  ]);

describe("iOS app icon set", () => {
  it("reads size and transparency from PNG headers", () => {
    expect(pngInfo(png({ size: 167 }))).toEqual({ width: 167, height: 167, hasAlpha: false });
    expect(pngInfo(png({ size: 20, colorType: 6 }))?.hasAlpha).toBe(true);
    expect(pngInfo(png({ size: 20, colorType: 4 }))?.hasAlpha).toBe(true);
    expect(pngInfo(png({ size: 20, colorType: 3, transparencyChunk: true }))?.hasAlpha).toBe(true);
    expect(pngInfo(Buffer.from("not a png"))).toBeNull();
  });

  it("accepts an opaque, complete set", () => {
    expect(iconProblems({ contents, files: goodFiles() })).toEqual([]);
  });

  // The upload App Store Connect refuses: RGBA icons, as every generator writes them.
  it("rejects an alpha channel", () => {
    const files = goodFiles().set("icon-1024.png", png({ size: 1024, colorType: 6 }));
    expect(iconProblems({ contents, files })).toEqual([
      "icon-1024.png has an alpha channel (ITMS-90717); run pnpm native:mobile:ios:icons.",
    ]);
  });

  it("rejects a missing file, a wrong size and an unreferenced file", () => {
    const files = goodFiles().set("icon-167.png", png({ size: 152 })).set("stray.png", png({ size: 20 }));
    files.delete("icon-180.png");
    expect(iconProblems({ contents, files })).toEqual([
      "icon-180.png (iphone 60x60@3x) is missing.",
      "icon-167.png is 152x152, expected 167x167 for ipad 83.5x83.5@2x.",
      "stray.png is not referenced by Contents.json.",
    ]);
  });

  it("requires exactly one 1024px marketing icon", () => {
    const withoutMarketing = { images: contents.images.slice(0, 2) };
    const files = goodFiles();
    files.delete("icon-1024.png");
    expect(iconProblems({ contents: withoutMarketing, files })).toEqual([
      "Expected exactly one 1024px ios-marketing icon, found 0.",
    ]);
  });

  it("holds for the icons the app ships", () => {
    expect(iconProblems(readIconSet(repoRootFromScript(import.meta.url)))).toEqual([]);
  });
});
