import { render, screen, userEvent, waitFor } from "@testing-library/react-native";
import { Pressable, Text } from "react-native";

import { localeReady } from "~/native/platform-boot";

import { useCopiedLabel } from "./use-copied-label";

beforeAll(async () => {
  await localeReady;
});

/**
 * The reset runs on a real timer with a short duration: React 19's render
 * scheduling cannot settle inside `act` under Jest fake timers, so advancing
 * the clock there never shows the label coming back.
 */
function Probe({ durationMs }: { durationMs: number }) {
  const { copied, markCopied } = useCopiedLabel(durationMs);
  return (
    <Pressable onPress={markCopied} testID="mark">
      <Text>{copied ? "COPIED" : "IDLE"}</Text>
    </Pressable>
  );
}

it("shows Copied and reverts after the duration", async () => {
  // Long enough to be seen: at 30 ms the label was back before the first
  // `findByText` poll whenever the machine was busy.
  await render(<Probe durationMs={400} />);
  const user = userEvent.setup();

  await user.press(screen.getByTestId("mark"));
  expect(await screen.findByText("COPIED")).toBeOnTheScreen();

  await waitFor(() => expect(screen.getByText("IDLE")).toBeOnTheScreen(), { timeout: 2000 });
});

it("re-arms when copied again inside the window", async () => {
  await render(<Probe durationMs={120} />);
  const user = userEvent.setup();

  await user.press(screen.getByTestId("mark"));
  expect(await screen.findByText("COPIED")).toBeOnTheScreen();
  // A second copy before the first expires restarts the hold, so the label
  // cannot flicker back to IDLE between two quick copies.
  await user.press(screen.getByTestId("mark"));
  await new Promise<void>((resolve) => {
    setTimeout(resolve, 60);
  });
  expect(screen.getByText("COPIED")).toBeOnTheScreen();

  await waitFor(() => expect(screen.getByText("IDLE")).toBeOnTheScreen(), { timeout: 2000 });
});
