import { StrictMode, useEffect } from "react";
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { useMountedRef } from "@/lib/use-mounted-ref";

describe("useMountedRef", () => {
  it("flips to false after unmount", () => {
    let observedRef: { current: boolean } | undefined;

    function Probe() {
      const mountedRef = useMountedRef();

      useEffect(() => {
        observedRef = mountedRef;
      }, [mountedRef]);

      return null;
    }

    const { unmount } = render(<Probe />);

    expect(observedRef).toBeDefined();
    expect(observedRef!.current).toBe(true);
    unmount();
    expect(observedRef!.current).toBe(false);
  });

  it("resets to true after Strict Mode effect replay", () => {
    const observedValues: boolean[] = [];

    function Probe() {
      const mountedRef = useMountedRef();

      useEffect(() => {
        observedValues.push(mountedRef.current);
      }, [mountedRef]);

      return null;
    }

    render(
      <StrictMode>
        <Probe />
      </StrictMode>,
    );

    expect(observedValues).toEqual([true, true]);
  });
});
