import { describe, expect, it } from "vitest";

import {
  DEFAULT_FONT,
  fontDefinitions,
  fontFromFamilyString,
  fontRoles,
  fontToClassName,
  fonts,
  MONO_FONT_STACK,
} from "@/config/fonts";

describe("font roles", () => {
  it("exposes a fixed monospace role resolving to the generic monospace family", () => {
    expect(MONO_FONT_STACK).toContain("ui-monospace");
    expect(MONO_FONT_STACK.trim().endsWith("monospace")).toBe(true);
    expect(fontRoles.mono).toBe(MONO_FONT_STACK);
  });

  it("maps the display role to Manrope and the body role to the runtime app font", () => {
    expect(fontRoles.display).toBe(fontDefinitions.manrope.css);
    expect(fontRoles.body).toBe("var(--app-font-family)");
  });

  it("keeps the user-selectable font set unchanged so selection stays intact", () => {
    expect(fonts).toEqual(["inter", "manrope", "system"]);
    expect(fonts).not.toContain("mono");
    expect(fontToClassName("manrope")).toBe("font-manrope");
  });

  it("still resolves persisted family strings so font persistence does not regress", () => {
    expect(fontFromFamilyString("Manrope")).toBe("manrope");
    expect(fontFromFamilyString("Inter")).toBe("inter");
    expect(fontFromFamilyString("System")).toBe("system");
    expect(fontFromFamilyString("")).toBe(DEFAULT_FONT);
  });
});
