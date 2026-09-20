import { render, screen } from "@testing-library/react-native";
import { WORLD_MAP_BOX, worldMapPosition } from "@voya/features/home/country-positions";
import { WORLD_MAP_LAND } from "@voya/features/home/world-map-land";

import { WorldMap } from "./world-map";

// The map is decoration, so it hides itself from accessibility — and that is
// exactly what the default queries filter out.
const hidden = { includeHiddenElements: true } as const;

describe("WorldMap", () => {
  it("draws the land whether or not there is anything to mark", async () => {
    await render(<WorldMap marker={null} />);

    expect(screen.getByTestId("world-map-land", hidden).props.d).toBe(WORLD_MAP_LAND);
    expect(screen.queryByTestId(/^world-map-marker-/, hidden)).toBeNull();
  });

  it("puts the marker where the projection says the country is", async () => {
    await render(<WorldMap marker={{ countryCode: "JP", state: "connected" }} />);

    const position = worldMapPosition("JP");
    expect(position).not.toBeNull();
    expect(screen.getByTestId("world-map-marker-JP", hidden)).toHaveStyle({
      left: `${position?.left}%`,
      top: `${position?.top}%`,
    });
  });

  it("ignores a country the map has no place for", async () => {
    await render(<WorldMap marker={{ countryCode: "ZZ", state: "selected" }} />);

    expect(screen.getByTestId("world-map-land", hidden)).toBeOnTheScreen();
    expect(screen.queryByTestId("world-map-marker-ZZ", hidden)).toBeNull();
  });

  it("keeps the projection's aspect ratio, or the marker would drift", async () => {
    await render(<WorldMap marker={null} />);

    expect(screen.root).toHaveStyle({
      aspectRatio: WORLD_MAP_BOX.width / WORLD_MAP_BOX.height,
    });
  });
});
