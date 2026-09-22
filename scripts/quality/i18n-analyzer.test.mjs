import { describe, expect, it } from "vitest";

import {
  EXTERNAL_KEY_NAMESPACES,
  inspectI18nSource,
  isUserVisibleText,
  unusedTranslationKeys,
} from "./i18n-analyzer.mjs";

const knownKeys = new Set(["actions.save", "form.placeholder"]);

function inspect(source) {
  return inspectI18nSource({ path: "fixture.tsx", source, knownKeys });
}

describe("i18n AST analyzer", () => {
  it("detects visible JSX attributes, expressions, object labels, and helper returns", () => {
    const result = inspect(`
      const choices = [{ label: "Automatic" }];
      function connectionStatusLabel() {
        return ready ? "Running" : "Stopped";
      }
      export function Fixture() {
        return <input aria-label="Server address" placeholder={ready ? "Ready" : "Waiting"} />;
      }
    `);

    expect(result.hardcodedText.map((item) => item.detail)).toEqual(
      expect.arrayContaining(["Automatic", "Running", "Stopped", "Server address", "Ready", "Waiting"]),
    );
  });

  it("accepts static locale calls and reports undefined or dynamic keys", () => {
    const result = inspect(`
      const valid = t("actions.save");
      const missing = t("actions.missing");
      const dynamic = t(\`actions.\${action}\`);
    `);

    expect(result.invalidKeys).toHaveLength(1);
    expect(result.invalidKeys[0]?.detail).toBe("actions.missing");
    expect(result.dynamicKeys).toHaveLength(1);
  });

  it("allows centralized technical literals", () => {
    expect(isUserVisibleText("sing-box")).toBe(false);
    expect(isUserVisibleText("AES-256-GCM")).toBe(false);
    expect(isUserVisibleText("https://example.com/config.json")).toBe(false);
    expect(isUserVisibleText("Save changes")).toBe(true);
  });

  // `{value || "Unknown"}` renders the fallback in every locale; before this
  // the analyzer descended into `+` only, so the whole family passed the gate.
  it("detects text rendered through ||, ?? and && fallbacks", () => {
    const result = inspect(`
      export function Fixture() {
        return (
          <div>
            <span>{strategy || "default"}</span>
            <span>{name ?? "Unnamed profile"}</span>
            <span>{loading && "Loading nodes"}</span>
            <span title={label || "Fallback title"} />
          </div>
        );
      }
    `);

    expect(result.hardcodedText.map((item) => item.detail).sort()).toEqual([
      "Fallback title",
      "Loading nodes",
      "Unnamed profile",
      "default",
    ]);
  });

  it("inspects both sides of || and ?? but only the rendered side of &&", () => {
    const result = inspect(`
      export function Fixture() {
        return (
          <div>
            <span>{"Primary label" || fallback}</span>
            <span>{"Never rendered" && other}</span>
          </div>
        );
      }
    `);

    expect(result.hardcodedText.map((item) => item.detail)).toEqual(["Primary label"]);
  });

  it("treats upper-case identifiers as technical but not upper-case words", () => {
    for (const technical of ["SOCKS5", "HTTP/2", "X25519", "AES-256", "TLS", "QUIC", "REALITY", "XHTTP", "KCP"]) {
      expect(isUserVisibleText(technical), technical).toBe(false);
    }
    // The old `^[A-Z][A-Z0-9_.+/-]{1,15}$` pattern accepted all of these.
    for (const visible of ["OK", "SAVE", "CANCEL", "ERROR", "RETRY"]) {
      expect(isUserVisibleText(visible), visible).toBe(true);
    }
  });

  it("flags hardcoded text in every shipped script, not only Latin and CJK", () => {
    expect(isUserVisibleText("Сохранить изменения")).toBe(true);
    expect(isUserVisibleText("ذخیره تغییرات")).toBe(true);
    expect(isUserVisibleText("保存")).toBe(true);
    expect(isUserVisibleText("1.2.3")).toBe(false);
    expect(isUserVisibleText("   ")).toBe(false);
  });

  it("counts a key as used from any string literal, including label maps", () => {
    const { literals } = inspect(`
      const labels = { running: "status.running" };
      const title = t("actions.save");
      const note = \`plain \${value}\`;
    `);

    expect(literals).toEqual(expect.arrayContaining(["status.running", "actions.save"]));
    expect(
      unusedTranslationKeys({
        keys: ["actions.save", "status.running", "status.stale", "startupFailure.title"],
        literals: new Set(literals),
        externalPrefixes: ["startupFailure."],
      }),
    ).toEqual(["status.stale"]);
  });

  it("names a reader for every namespace read outside the frontend", () => {
    for (const namespace of EXTERNAL_KEY_NAMESPACES) {
      expect(namespace.prefix.endsWith("."), namespace.prefix).toBe(true);
      expect(namespace.reader).toMatch(/^(crates|apps)\//);
      expect(namespace.marker.length).toBeGreaterThan(0);
    }
  });
});
