import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { PageHeader, PageSection, PageTitle } from "./page-section";

describe("PageSection primitives", () => {
  it("renders the shared section/header geometry with stable data-slots", () => {
    const { getByTestId } = render(
      <PageSection aria-label="Logs" data-testid="section">
        <PageHeader data-testid="header" />
      </PageSection>,
    );

    const section = getByTestId("section");
    expect(section.dataset.slot).toBe("page-section");
    // The full-height, min-h-0 flex column that every screen previously hand-wrote.
    expect(section.className).toContain("flex h-full min-h-0 flex-col");
    expect(section.getAttribute("aria-label")).toBe("Logs");

    const header = getByTestId("header");
    expect(header.dataset.slot).toBe("page-header");
    // The canonical header spacing scale: 56px tall, px-4 py-2 padding, gap-2 items.
    expect(header.className).toContain("min-h-14");
    expect(header.className).toContain("px-4");
    expect(header.className).toContain("py-2");
    expect(header.className).toContain("gap-2");
  });

  it("renders the large page identity as the page's single h1 with count and actions", () => {
    const { getByRole, getByTestId, getByText } = render(
      <PageTitle
        actions={<button type="button">New</button>}
        count="12"
        data-testid="title"
        title="Nodes"
      />,
    );

    const title = getByTestId("title");
    expect(title.dataset.slot).toBe("page-title");
    expect(title.className).toContain("min-[1100px]:px-page");

    const heading = getByRole("heading", { level: 1, name: "Nodes" });
    expect(heading.className).toContain("text-page");
    expect(getByText("12")).toBeInTheDocument();
    // Actions park at the trailing edge via the logical ms-auto push.
    expect(
      getByRole("button", { name: "New" }).parentElement?.className,
    ).toContain("ms-auto");
  });
});
