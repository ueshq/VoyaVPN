import { definePlugin, defineRule } from "vite-plus/lint/plugins";

// ADR-0002: only apps/desktop/src/ipc may reach Tauri. `no-restricted-imports`
// in vite.config.ts covers static import/export declarations; these two rules
// cover what it cannot see, which ESLint expressed as `no-restricted-syntax`
// selectors and Oxlint does not implement.

const TAURI_PACKAGE = /^@tauri-apps/;
const TAURI_GLOBAL = /^__TAURI/;

function isTauriSpecifier(node) {
  return node?.type === "Literal" && typeof node.value === "string" && TAURI_PACKAGE.test(node.value);
}

// `import("@tauri-apps/…")` and `require("@tauri-apps/…")`.
const noTauriDynamicImport = defineRule({
  meta: {
    type: "problem",
    messages: { boundary: "ADR-0002: only apps/desktop/src/ipc may import Tauri APIs." },
  },
  create(context) {
    return {
      ImportExpression(node) {
        if (isTauriSpecifier(node.source)) context.report({ node: node.source, messageId: "boundary" });
      },
      CallExpression(node) {
        if (node.callee.type !== "Identifier" || node.callee.name !== "require") return;
        const [specifier] = node.arguments;
        if (isTauriSpecifier(specifier)) context.report({ node: specifier, messageId: "boundary" });
      },
    };
  },
});

// Reaching window.__TAURI_INTERNALS__ bypasses the typed wrappers just like a
// direct import does. A `"__TAURI_INTERNALS__" in window` presence probe is not
// a member access, so it stays allowed.
const noTauriGlobals = defineRule({
  meta: {
    type: "problem",
    messages: { boundary: "ADR-0002: only apps/desktop/src/ipc may touch the Tauri runtime globals." },
  },
  create(context) {
    return {
      MemberExpression(node) {
        const { property } = node;
        const name = node.computed
          ? property.type === "Literal" && typeof property.value === "string"
            ? property.value
            : null
          : property.type === "Identifier"
            ? property.name
            : null;
        if (name !== null && TAURI_GLOBAL.test(name)) context.report({ node: property, messageId: "boundary" });
      },
    };
  },
});

export default definePlugin({
  meta: { name: "voya" },
  rules: {
    "no-tauri-dynamic-import": noTauriDynamicImport,
    "no-tauri-globals": noTauriGlobals,
  },
});
