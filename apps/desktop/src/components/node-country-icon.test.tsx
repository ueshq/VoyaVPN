import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { NodeCountryIcon } from "./node-country-icon";

afterEach(cleanup);

describe("NodeCountryIcon", () => {
  it.each(["US", " jp ", "HK", "TW", "DE"])("renders the bundled flag for %s", (countryCode) => {
    const code = countryCode.trim().toLowerCase();
    const { container } = render(<NodeCountryIcon countryCode={countryCode} />);
    const flag = container.querySelector<HTMLElement>(`.fi-${code}`);
    expect(flag).toBeInTheDocument();
    expect(flag?.style.backgroundImage).toContain(`/flags/4x3/${code}.svg`);
    expect(container.querySelector("svg")).toBeNull();
  });

  it.each([null, undefined, "", "XX", "ZZ", "US injected", "🇯🇵 Japan", "USA"])("uses the globe for %s", (countryCode) => {
    const { container } = render(<NodeCountryIcon countryCode={countryCode} />);
    expect(container.querySelector("svg")).toBeInTheDocument();
    expect(container.querySelector(".fi")).toBeNull();
  });
});
