import { RuleTester } from "vite-plus/lint/plugins-dev";
import { describe, it } from "vite-plus/test";

import plugin from "./voya-plugin.mjs";

RuleTester.describe = describe;
RuleTester.it = it;

const tester = new RuleTester({ languageOptions: { sourceType: "module" } });

tester.run("voya/no-tauri-dynamic-import", plugin.rules["no-tauri-dynamic-import"], {
  valid: [
    'const ui = await import("@voya/ui/button");',
    'const path = require("node:path");',
    'load("@tauri-apps/api/core");',
  ],
  invalid: [
    { code: 'const core = await import("@tauri-apps/api/core");', errors: [{ messageId: "boundary" }] },
    { code: 'const dialog = require("@tauri-apps/plugin-dialog");', errors: [{ messageId: "boundary" }] },
  ],
});

tester.run("voya/no-tauri-globals", plugin.rules["no-tauri-globals"], {
  valid: [
    'const inShell = "__TAURI_INTERNALS__" in window;',
    "window.voya = 1;",
    "const key = '__TAURI_INTERNALS__'; window[key];",
  ],
  invalid: [
    { code: "window.__TAURI_INTERNALS__.invoke('x');", errors: [{ messageId: "boundary" }] },
    { code: 'globalThis["__TAURI__"].core;', errors: [{ messageId: "boundary" }] },
  ],
});
