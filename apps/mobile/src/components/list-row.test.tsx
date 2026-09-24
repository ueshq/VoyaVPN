import { render, screen, userEvent } from "@testing-library/react-native";
import type { ReactNode } from "react";
import { Text } from "react-native";

import { ListCard } from "./list-card";
import { ListRow } from "./list-row";
import { withListPositions } from "./list-positions";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

/** HeroUI's components read their provider; nothing here queries. */
function wrapper({ children }: { children: ReactNode }) {
  return <TestProviders queryClient={makeTestQueryClient()}>{children}</TestProviders>;
}

describe("ListRow", () => {
  it("is a button carrying its selected state when it can be pressed", async () => {
    const onPress = jest.fn();
    await render(
      <ListCard>
        <ListRow title="English" onPress={onPress} accessibilityState={{ selected: true }} />
        <ListRow last title="简体中文" onPress={() => {}} accessibilityState={{ selected: false }} />
      </ListCard>,
      { wrapper },
    );

    expect(screen.getByRole("button", { name: "English" })).toBeSelected();
    expect(screen.getByRole("button", { name: "简体中文" })).not.toBeSelected();
    await userEvent.setup().press(screen.getByText("English"));
    expect(onPress).toHaveBeenCalledTimes(1);
  });

  it("is plain text with its trailing control when it cannot be pressed", async () => {
    await render(
      <ListRow title="FakeIP" description="Answers with placeholder addresses" trailing={<Text>switch</Text>} />,
      { wrapper },
    );

    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("Answers with placeholder addresses")).toBeOnTheScreen();
    expect(screen.getByText("switch")).toBeOnTheScreen();
  });
});

describe("withListPositions", () => {
  it("marks the first and last items, and a single item as both", () => {
    expect(withListPositions(["a", "b", "c"]).map(({ first, last }) => [first, last])).toEqual([
      [true, false],
      [false, false],
      [false, true],
    ]);
    expect(withListPositions(["only"])).toEqual([{ first: true, item: "only", last: true }]);
    expect(withListPositions([])).toEqual([]);
  });
});
