import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Card } from "./card";

describe("Card elevation", () => {
  it("rests on the depth-ladder shadow token instead of the flat tailwind shadow", () => {
    const { getByTestId } = render(<Card data-testid="card">content</Card>);
    const card = getByTestId("card");

    expect(card.dataset.slot).toBe("card");
    expect(card.className).toContain("shadow-[var(--shadow-sm)]");
  });
});
