import { render, screen, userEvent } from "@testing-library/react-native";
import type { ReactNode } from "react";

import { SegmentedControl } from "./segmented-control";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

/** HeroUI's components read their provider; nothing here queries. */
function wrapper({ children }: { children: ReactNode }) {
  return <TestProviders queryClient={makeTestQueryClient()}>{children}</TestProviders>;
}

const OPTIONS = [
  { label: "Rule", value: "rule" },
  { label: "Global", value: "global" },
] as const;

describe("SegmentedControl", () => {
  it("marks only the chosen segment selected and reports a new choice", async () => {
    const onChange = jest.fn();
    await render(<SegmentedControl options={OPTIONS} value="rule" onChange={onChange} />, { wrapper });

    expect(screen.getByRole("button", { name: "Rule" })).toBeSelected();
    expect(screen.getByRole("button", { name: "Global" })).not.toBeSelected();

    await userEvent.setup().press(screen.getByRole("button", { name: "Global" }));
    expect(onChange).toHaveBeenCalledWith("global");
  });

  it("offers nothing to press while disabled", async () => {
    const onChange = jest.fn();
    await render(<SegmentedControl options={OPTIONS} value="rule" onChange={onChange} isDisabled />, { wrapper });

    expect(screen.getByRole("button", { name: "Global" })).toBeDisabled();
  });
});
