import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { PageHeader, PageSection, PageTitle } from "./page-section";
import { WindowChromeContext } from "./window-chrome-context";

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
    expect(section.className).toContain("flex h-full min-h-0 min-w-0 flex-col");
    expect(section.getAttribute("aria-label")).toBe("Logs");

    const header = getByTestId("header");
    expect(header.dataset.slot).toBe("page-header");
    // Panel tools use the same compact row and local padding.
    expect(header.className).toContain("min-h-12");
    expect(header.className).toContain("px-4");
    expect(header.className).toContain("py-2");
    expect(header.className).toContain("gap-2");
  });

  it("renders the large page identity as the page's single h1 with actions", () => {
    const { getByRole, getByTestId } = render(
      <PageTitle
        actions={<button type="button">New</button>}
        data-testid="title"
        title="Nodes"
      />,
    );

    const title = getByTestId("title");
    expect(title.dataset.slot).toBe("page-title");
    expect(title.className).toContain("min-[1100px]:px-page");
    // Equal padding keeps the title as far from the top as from the content.
    expect(title.className).toContain("py-4");

    const heading = getByRole("heading", { level: 1, name: "Nodes" });
    expect(heading.className).toContain("text-page");
    // Actions park at the trailing edge via the logical ms-auto push.
    expect(
      getByRole("button", { name: "New" }).parentElement?.className,
    ).toContain("ms-auto");
  });

  it("drags the window from the title row only under macOS chrome", () => {
    const { getByTestId } = render(
      <>
        <WindowChromeContext value="macos">
          <PageTitle data-testid="macos" title="Nodes" />
        </WindowChromeContext>
        <WindowChromeContext value="windows">
          <PageTitle data-testid="windows" title="Rules" />
        </WindowChromeContext>
        <PageTitle data-testid="none" title="Settings" />
      </>,
    );

    expect(getByTestId("macos")).toHaveAttribute("data-tauri-drag-region", "deep");
    expect(getByTestId("windows")).not.toHaveAttribute("data-tauri-drag-region");
    expect(getByTestId("none")).not.toHaveAttribute("data-tauri-drag-region");
  });
});
