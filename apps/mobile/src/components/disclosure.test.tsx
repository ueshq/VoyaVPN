import { render, screen, userEvent } from "@testing-library/react-native";
import { ListGroup } from "heroui-native/list-group";
import type { ReactNode } from "react";
import { Text } from "react-native";

import { Disclosure } from "./disclosure";
import { SwitchRow } from "./switch-row";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

/** HeroUI's components read their provider; nothing here queries. */
function wrapper({ children }: { children: ReactNode }) {
  return <TestProviders queryClient={makeTestQueryClient()}>{children}</TestProviders>;
}

describe("Disclosure", () => {
  it("is a button that says whether it is open, and mounts its content only when it is", async () => {
    await render(<Disclosure title="Licenses"><Text>MIT</Text></Disclosure>, { wrapper });

    const toggle = screen.getByRole("button", { name: "Licenses" });
    expect(toggle).not.toBeExpanded();
    expect(screen.queryByText("MIT")).toBeNull();

    await userEvent.setup().press(toggle);
    expect(toggle).toBeExpanded();
    expect(screen.getByText("MIT")).toBeOnTheScreen();
  });

  it("stays open while its owner holds it open", async () => {
    const onExpandedChange = jest.fn();
    await render(
      <Disclosure title="Advanced" isExpanded onExpandedChange={onExpandedChange}><Text>Remote DNS</Text></Disclosure>,
      { wrapper },
    );

    await userEvent.setup().press(screen.getByRole("button", { name: "Advanced" }));
    expect(onExpandedChange).toHaveBeenCalledWith(false);
    expect(screen.getByText("Remote DNS")).toBeOnTheScreen();
  });
});

describe("SwitchRow", () => {
  it("keeps the switch the named control a screen reader operates", async () => {
    const onChange = jest.fn();
    await render(
      <ListGroup><SwitchRow last label="FakeIP" value={false} onChange={onChange} /></ListGroup>,
      { wrapper },
    );

    const control = screen.getByRole("switch", { name: "FakeIP" });
    expect(control).not.toBeChecked();
    await userEvent.setup().press(control);
    expect(onChange).toHaveBeenCalledWith(true);
  });
});
