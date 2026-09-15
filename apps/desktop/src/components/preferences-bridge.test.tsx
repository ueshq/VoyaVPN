import { cleanup, waitFor } from "@testing-library/react";
import { createTestQueryClient, renderWithQuery } from "@/test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { PreferencesBridge } from "@/components/preferences-bridge";
import { changeLocale } from "@voya/i18n";
import { queryKeys } from "@/ipc/query-keys";
import { usePreferencesStore } from "@/stores/preferences-store";

const preferencesMocks = vi.hoisted(() => ({
  loadUiPreferences: vi.fn(),
}));

vi.mock("@/ipc/commands", () => preferencesMocks);

describe("PreferencesBridge", () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await changeLocale("en");
    usePreferencesStore.getState().setThemeMode("system");
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: vi.fn(() => ({
        addEventListener: vi.fn(),
        matches: false,
        removeEventListener: vi.fn(),
      })),
    });
  });

  afterEach(async () => {
    cleanup();
    document.documentElement.classList.remove("dark");
    document.documentElement.style.colorScheme = "";
    await changeLocale("en");
  });

  it("applies backend theme, locale, and direction again after cross-window invalidation", async () => {
    const queryClient = createTestQueryClient();
    preferencesMocks.loadUiPreferences.mockResolvedValueOnce({ language: "zh-Hant", theme: "dark" });

    renderWithQuery(<PreferencesBridge />, { queryClient });

    await waitFor(() => {
      expect(document.documentElement).toHaveClass("dark");
      expect(document.documentElement).toHaveAttribute("lang", "zh-Hant");
      expect(document.documentElement).toHaveAttribute("dir", "ltr");
    });
    expect(usePreferencesStore.getState().themeMode).toBe("dark");

    preferencesMocks.loadUiPreferences.mockResolvedValueOnce({ language: "en", theme: "light" });
    await queryClient.invalidateQueries({ queryKey: queryKeys.uiPreferences });

    await waitFor(() => {
      expect(document.documentElement).not.toHaveClass("dark");
      expect(document.documentElement.style.colorScheme).toBe("light");
      expect(document.documentElement).toHaveAttribute("lang", "en");
      expect(document.documentElement).toHaveAttribute("dir", "ltr");
    });
    expect(usePreferencesStore.getState().themeMode).toBe("light");
  });
});
