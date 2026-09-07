import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AppErrorBoundary } from "./error-boundary";

function Boom(): never {
  throw new Error("screen exploded");
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("AppErrorBoundary", () => {
  beforeEach(() => {
    // React re-logs every caught render error; keep the test output readable.
    vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  it("renders its children while nothing throws", () => {
    render(
      <AppErrorBoundary>
        <p>screen content</p>
      </AppErrorBoundary>,
    );

    expect(screen.getByText("screen content")).toBeInTheDocument();
    expect(screen.queryByTestId("app-error-fallback")).not.toBeInTheDocument();
  });

  it("contains a throwing screen and offers a localized retry", async () => {
    const user = userEvent.setup();
    let shouldThrow = true;

    function Screen() {
      if (shouldThrow) {
        return <Boom />;
      }

      return <p>recovered screen</p>;
    }

    render(
      <div>
        <nav aria-label="Main sections">sidebar</nav>
        <AppErrorBoundary>
          <Screen />
        </AppErrorBoundary>
      </div>,
    );

    // The shell around the crashed screen must survive.
    expect(screen.getByRole("navigation", { name: "Main sections" })).toBeInTheDocument();
    const fallback = screen.getByRole("alert");
    expect(fallback).toHaveTextContent("Something went wrong");
    expect(fallback).toHaveTextContent(
      "This screen stopped unexpectedly. Try again, or reload the app if it keeps happening.",
    );

    shouldThrow = false;
    await user.click(screen.getByRole("button", { name: "Try again" }));

    expect(screen.getByText("recovered screen")).toBeInTheDocument();
    expect(screen.queryByTestId("app-error-fallback")).not.toBeInTheDocument();
  });

  it("clears the caught error when the reset key changes", () => {
    const { rerender } = render(
      <AppErrorBoundary resetKey="home">
        <Boom />
      </AppErrorBoundary>,
    );

    expect(screen.getByTestId("app-error-fallback")).toBeInTheDocument();

    rerender(
      <AppErrorBoundary resetKey="profiles">
        <p>other screen</p>
      </AppErrorBoundary>,
    );

    expect(screen.getByText("other screen")).toBeInTheDocument();
    expect(screen.queryByTestId("app-error-fallback")).not.toBeInTheDocument();
  });
});
