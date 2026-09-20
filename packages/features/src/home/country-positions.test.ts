import { describe, expect, it } from "vitest";

import { worldMapPosition } from "./country-positions";

// Landmass outlines measured from world-map.svg, in its 1078 × 466 coordinates.
describe("worldMapPosition", () => {
  it.each([
    ["JP", 912.7, 949.1, 153.3, 183.1],
    ["GB", 518.1, 540.9, 103.7, 128.7],
    ["AU", 865.9, 983.1, 303.0, 384.6],
  ] as const)("places %s on its own landmass", (code, minX, maxX, minY, maxY) => {
    const position = worldMapPosition(code);
    expect(position).not.toBeNull();
    const x = ((position?.left ?? 0) / 100) * 1078;
    const y = ((position?.top ?? 0) / 100) * 466;
    expect(x).toBeGreaterThanOrEqual(minX);
    expect(x).toBeLessThanOrEqual(maxX);
    expect(y).toBeGreaterThanOrEqual(minY);
    expect(y).toBeLessThanOrEqual(maxY);
  });

  it("accepts any letter case and ignores unknown or missing codes", () => {
    expect(worldMapPosition(" jp ")).toEqual(worldMapPosition("JP"));
    expect(worldMapPosition("ZZ")).toBeNull();
    expect(worldMapPosition("")).toBeNull();
    expect(worldMapPosition(null)).toBeNull();
  });
});
