import { render, screen, userEvent } from "@testing-library/react-native";
import type { ReactNode } from "react";

import { SectionHeader } from "./section-header";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

/** HeroUI's components read their provider; nothing here queries. */
function wrapper({ children }: { children: ReactNode }) {
  return <TestProviders queryClient={makeTestQueryClient()}>{children}</TestProviders>;
}

describe("SectionHeader", () => {
  it("is a heading when it only names a section", async () => {
    await render(<SectionHeader title="Language" />, { wrapper });

    expect(screen.getByRole("header")).toBeOnTheScreen();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("is a disclosure button that says whether the section is open", async () => {
    const onToggle = jest.fn();
    await render(<SectionHeader title="Local nodes" detail="3 nodes" expanded onToggle={onToggle} />, { wrapper });

    const toggle = screen.getByRole("button", { name: /Local nodes/ });
    expect(toggle).toBeExpanded();
    expect(screen.getByText("3 nodes")).toBeOnTheScreen();

    await userEvent.setup().press(toggle);
    expect(onToggle).toHaveBeenCalledTimes(1);
  });
});
