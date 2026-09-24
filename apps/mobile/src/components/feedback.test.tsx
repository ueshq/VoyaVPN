import { render, screen, userEvent } from "@testing-library/react-native";
import type { ReactNode } from "react";
import { Server } from "lucide-react-native";
import { Pressable, Text } from "react-native";

import { Banner } from "./banner";
import { EmptyState } from "./empty-state";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

/** HeroUI's components read their provider; nothing here queries. */
function wrapper({ children }: { children: ReactNode }) {
  return <TestProviders queryClient={makeTestQueryClient()}>{children}</TestProviders>;
}

describe("Banner", () => {
  it("shows its message with the one action it carries", async () => {
    const retry = jest.fn();
    await render(
      <Banner
        status="danger"
        message="Could not connect: timed out"
        action={<Pressable accessibilityRole="button" onPress={retry}><Text>Retry</Text></Pressable>}
      />,
      { wrapper },
    );

    expect(screen.getByText("Could not connect: timed out")).toBeOnTheScreen();
    await userEvent.setup().press(screen.getByRole("button", { name: "Retry" }));
    expect(retry).toHaveBeenCalledTimes(1);
  });
});

describe("EmptyState", () => {
  it("says what is missing and how to fill it, with its icons hidden from assistive technology", async () => {
    await render(
      <EmptyState
        icons={[Server]}
        title="No nodes"
        description="Add a node or import one from a subscription to get started."
      />,
      { wrapper },
    );

    expect(screen.getByText("No nodes")).toBeOnTheScreen();
    expect(screen.getByText("Add a node or import one from a subscription to get started.")).toBeOnTheScreen();
    expect(screen.queryByRole("image")).toBeNull();
  });
});
