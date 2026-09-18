// @vitest-environment jsdom
import { useEffect } from "react";
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { useLatestRef } from "./use-latest-ref";

describe("useLatestRef", () => {
  it("keeps one ref that follows the latest render", () => {
    const observed: Array<{ current: string }> = [];

    function Probe({ value }: { value: string }) {
      const ref = useLatestRef(value);

      useEffect(() => {
        observed.push(ref);
      });

      return null;
    }

    const { rerender } = render(<Probe value="first" />);

    expect(observed.at(-1)?.current).toBe("first");
    rerender(<Probe value="second" />);
    expect(observed.at(-1)?.current).toBe("second");
    expect(observed.at(-1)).toBe(observed[0]);
  });
});
