import { contentColumn } from "./content-column";

test("leaves every iPhone width alone", () => {
  for (const width of [320, 390, 440]) {
    expect(contentColumn(width)).toBeUndefined();
  }
});

test("centres the content in a column on an iPad", () => {
  expect(contentColumn(834)).toEqual({ alignSelf: "center", maxWidth: 640, width: "100%" });
  // A sheet asks for a narrower column than a page.
  expect(contentColumn(1024, 480)).toEqual({ alignSelf: "center", maxWidth: 480, width: "100%" });
});

test("uses the phone layout in a narrow iPad split view", () => {
  expect(contentColumn(507)).toBeUndefined();
});
