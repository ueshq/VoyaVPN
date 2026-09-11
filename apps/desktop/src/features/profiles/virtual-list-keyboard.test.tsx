import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { navigateVirtualList } from "./virtual-list-keyboard";

let frames: FrameRequestCallback[];
const scroll = vi.fn();

function List({ count = 3, prevent = false }: { count?: number; prevent?: boolean }) {
  return (
    <div data-testid="viewport" tabIndex={0} onKeyDown={(event) => {
      if (prevent) event.preventDefault();
      navigateVirtualList(event, count, scroll, "data-row", "button");
    }}>
      {[0, 1, 2].map((index) => (
        <div data-row={index} key={index}><button>{index}</button></div>
      ))}
      <input aria-label="search" />
      <textarea aria-label="notes" />
      <div role="menu"><button>menu item</button></div>
    </div>
  );
}

describe("virtual list keyboard navigation", () => {
  beforeEach(() => {
    frames = [];
    scroll.mockReset();
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
  });
  afterEach(() => { cleanup(); vi.restoreAllMocks(); });

  it.each([
    ["ArrowDown", "0", 1], ["ArrowUp", "2", 1],
    ["Home", "2", 0], ["End", "0", 2],
    ["ArrowUp", "0", 0], ["ArrowDown", "2", 2],
  ])("%s scrolls before restoring focus from row %s", (key, from, next) => {
    render(<List />);
    const source = screen.getByRole("button", { name: from });
    source.focus();
    fireEvent.keyDown(source, { key });
    expect(scroll).toHaveBeenCalledExactlyOnceWith(next);
    expect(document.activeElement).toBe(source);
    frames.forEach((callback) => callback(0));
    expect(document.activeElement).toBe(screen.getByRole("button", { name: String(next) }));
  });

  it("enters the first row from the viewport", () => {
    render(<List />);
    fireEvent.keyDown(screen.getByTestId("viewport"), { key: "ArrowDown" });
    expect(scroll).toHaveBeenCalledExactlyOnceWith(0);
  });

  it("tolerates a row disappearing before the animation frame", () => {
    const { unmount } = render(<List />);
    fireEvent.keyDown(screen.getByTestId("viewport"), { key: "End" });
    unmount();
    expect(() => frames.forEach((callback) => callback(0))).not.toThrow();
  });

  it.each(["search", "notes", "menu item"])("leaves %s keyboard behavior alone", (name) => {
    render(<List />);
    const element = name === "menu item" ? screen.getByRole("button", { name }) : screen.getByLabelText(name);
    fireEvent.keyDown(element, { key: "ArrowDown" });
    expect(scroll).not.toHaveBeenCalled();
    expect(frames).toHaveLength(0);
  });

  it.each([{ altKey: true }, { ctrlKey: true }, { metaKey: true }, { key: "Enter" }])("ignores unrelated or modified keys %j", (options) => {
    render(<List />);
    fireEvent.keyDown(screen.getByTestId("viewport"), { key: "ArrowDown", ...options });
    expect(scroll).not.toHaveBeenCalled();
  });

  it.each([{ count: 0 }, { prevent: true }])("ignores empty lists and handled events %j", (props) => {
    render(<List {...props} />);
    fireEvent.keyDown(screen.getByTestId("viewport"), { key: "ArrowDown" });
    expect(scroll).not.toHaveBeenCalled();
  });
});
