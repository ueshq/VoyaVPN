import { render, screen, userEvent } from "@testing-library/react-native";
import { Typography } from "heroui-native/text";
import type { ReactNode } from "react";

import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

import { guarded } from "./guarded";

function wrapper({ children }: { children: ReactNode }) {
  return <TestProviders queryClient={makeTestQueryClient()}>{children}</TestProviders>;
}

beforeAll(async () => {
  await localeReady;
});

it("replaces a screen that throws with a retry, and shows the screen again once it renders", async () => {
  // React logs the caught error; the boundary logs it once more.
  const logged = jest.spyOn(console, "error").mockImplementation(() => undefined);
  let broken = true;
  const Screen = guarded(function Screen({ title }: { title: string }) {
    if (broken) throw new Error("render failed");
    return <Typography>{title}</Typography>;
  });

  await render(<Screen title="Nodes" />, { wrapper });

  expect(screen.getByText("Something went wrong")).toBeOnTheScreen();
  expect(screen.queryByText("Nodes")).toBeNull();

  broken = false;
  await userEvent.setup().press(screen.getByText("Try again"));

  expect(screen.getByText("Nodes")).toBeOnTheScreen();
  expect(screen.queryByTestId("screen-error-fallback")).toBeNull();
  logged.mockRestore();
});
