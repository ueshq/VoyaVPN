import { WORLD_MAP_BOX } from "@voya/features/home/country-positions";
import { WORLD_MAP_LAND } from "@voya/features/home/world-map-land";
import { describe, expect, it } from "vitest";

import svg from "../../assets/world-map.svg?raw";

/**
 * The asset and the shared path are one map with two renderers.
 *
 * This app paints `world-map.svg` as a CSS mask; React Native has no mask and
 * no SVG loader, so it draws `WORLD_MAP_LAND` with `react-native-svg`. Nothing
 * else keeps the two in step, and the projection in `country-positions.ts`
 * only places markers correctly on the outline it was fitted to — so an edit
 * to either one has to be an edit to both.
 */
describe("world map asset", () => {
  it("is drawn in the box the projection places markers in", () => {
    expect(svg).toContain(
      `viewBox="0 0 ${WORLD_MAP_BOX.width} ${WORLD_MAP_BOX.height}"`,
    );
  });

  it("holds exactly the land the shared path draws", () => {
    // One filled path, so "the outline" is unambiguous on both platforms.
    const paths = [...svg.matchAll(/ d="([^"]+)"/g)].map(([, d]) => d);
    expect(paths).toContain(WORLD_MAP_LAND);
  });
});
