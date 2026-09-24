import { render, screen, userEvent } from "@testing-library/react-native";
import { IpcCommandError } from "@voya/client/errors";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";
import type { ReactNode } from "react";
import { ErrorNotice } from "./error-notice";

function wrapper({ children }: { children: ReactNode }) { return <TestProviders queryClient={makeTestQueryClient()}>{children}</TestProviders>; }

beforeAll(async () => { await localeReady; });

it("uses a typed recovery message and keeps redacted diagnostics behind disclosure", async () => {
  const error = new IpcCommandError({ kind: { type: "network" }, subsystem: "subscription", message: "GET https://example.test/sub?token=private-token failed" });
  await render(<ErrorNotice error={error} />, { wrapper });
  expect(screen.getByText("The network request failed. Check your connection and source address, then retry.")).toBeOnTheScreen();
  expect(screen.queryByText(/GET/)).toBeNull();
  await userEvent.setup().press(screen.getByText("Technical details"));
  expect(screen.getByText(/GET/).props.children).not.toContain("private-token");
});

it("preserves field-specific save guidance over a general reason", async () => {
  await render(<ErrorNotice error={new Error("database diagnostic")} reason="database" message="The previous settings remain in use." />, { wrapper });
  expect(screen.getByText("The previous settings remain in use.")).toBeOnTheScreen();
  expect(screen.queryByText("database diagnostic")).toBeNull();
});
