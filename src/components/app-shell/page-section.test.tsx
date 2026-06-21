import { render } from "@testing-library/react";
import { ScrollText } from "lucide-react";
import { describe, expect, it } from "vitest";

import { Badge } from "@/components/ui/badge";

import { PageHeader, PageHeaderHeading, PageSection } from "./page-section";

describe("PageSection primitives", () => {
  it("renders the shared section/header geometry with stable data-slots", () => {
    const { getByTestId, getByRole } = render(
      <PageSection aria-label="Logs" data-testid="section">
        <PageHeader data-testid="header">
          <PageHeaderHeading icon={ScrollText} title="Logs">
            <Badge>3</Badge>
          </PageHeaderHeading>
        </PageHeader>
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

    // The heading exposes the title as a level-2 landmark so screens stay accessible.
    expect(getByRole("heading", { level: 2, name: "Logs" })).toBeInTheDocument();
  });

  it("merges extra classes onto the heading cluster via cn()", () => {
    const { getByTestId } = render(
      <PageHeaderHeading className="ms-auto" data-testid="heading" title="DNS" />,
    );

    const heading = getByTestId("heading");
    expect(heading.dataset.slot).toBe("page-header-heading");
    expect(heading.className).toContain("flex min-w-0 items-center gap-2");
    expect(heading.className).toContain("ms-auto");
  });
});
