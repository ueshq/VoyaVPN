import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

const capability = JSON.parse(
  readFileSync(
    new URL("../../apps/desktop/src-tauri/capabilities/default.json", import.meta.url),
    "utf8",
  ),
);

describe("desktop window capabilities", () => {
  it("scopes the capability to the single main window with caption-button permissions", () => {
    expect(capability.windows).toEqual(["main"]);
    expect(capability.permissions).toEqual(
      expect.arrayContaining([
        "core:window:allow-minimize",
        "core:window:allow-toggle-maximize",
        "core:window:allow-close",
        "core:window:allow-start-dragging",
      ]),
    );
  });
});
