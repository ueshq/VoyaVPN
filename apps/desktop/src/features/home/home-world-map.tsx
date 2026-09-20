import { worldMapPosition } from "@voya/features/home/country-positions";

export type HomeMapMarker = {
  countryCode: string;
  /** Connected: where traffic leaves. Selected: where it would leave. */
  state: "connected" | "selected";
};

/**
 * The world map behind Home's hero. The land is a mask painted with a theme
 * token; when the exit country is known it is marked with a dot.
 */
export function HomeWorldMap({ marker }: { marker: HomeMapMarker | null }) {
  const position = marker ? worldMapPosition(marker.countryCode) : null;
  return (
    <div aria-hidden="true" className="home-map-layer">
      <div className="home-world-map">
        {marker && position ? (
          <span
            className="home-map-marker"
            data-country={marker.countryCode.trim().toUpperCase()}
            data-state={marker.state}
            style={{ left: `${position.left}%`, top: `${position.top}%` }}
          />
        ) : null}
      </div>
    </div>
  );
}
