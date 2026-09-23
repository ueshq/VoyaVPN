import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

import { describe, expect, it } from "vitest";

// The desktop Vitest project runs under jsdom, where `import.meta.url` is an
// http:// URL and `fileURLToPath` throws. Resolve the capability file from the
// working directory instead (workspace root or apps/desktop).
function readCapability() {
  const candidates = [
    join(process.cwd(), "apps/desktop/src-tauri/capabilities/default.json"),
    join(process.cwd(), "src-tauri/capabilities/default.json"),
  ];
  for (const path of candidates) {
    if (existsSync(path)) {
      return JSON.parse(readFileSync(path, "utf8"));
    }
  }
  throw new Error(`capabilities/default.json not found from ${process.cwd()}`);
}

const capability = readCapability();

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
