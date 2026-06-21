import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Button } from "./button";

describe("Button signal variant", () => {
  it("applies the signal accent and connection glow primitives", () => {
    const { getByRole } = render(<Button variant="signal">Connect</Button>);
    const button = getByRole("button", { name: "Connect" });

    expect(button.className).toContain("bg-signal");
    expect(button.className).toContain("text-signal-foreground");
    expect(button.className).toContain("shadow-[var(--glow-signal)]");
    expect(button.className).toContain("focus-visible:ring-signal/40");
  });

  it("leaves the default variant untouched", () => {
    const { getByRole } = render(<Button>Default</Button>);
    const button = getByRole("button", { name: "Default" });

    expect(button.className).toContain("bg-primary");
    expect(button.className).toContain("text-primary-foreground");
    expect(button.className).not.toContain("bg-signal");
    expect(button.className).not.toContain("var(--glow-signal)");
  });
});
