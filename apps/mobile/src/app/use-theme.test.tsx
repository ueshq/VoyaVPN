import { act, renderHook } from "@testing-library/react-native";
import { Uniwind } from "uniwind";
import { Appearance, Platform } from "react-native";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { useTheme } from "./use-theme";

afterEach(() => jest.restoreAllMocks());

test("keeps native controls and Uniwind in sync and releases both overrides for system theme", async () => {
  const setTheme = jest.spyOn(Uniwind, "setTheme");
  const setColorScheme = jest.spyOn(Appearance, "setColorScheme");
  usePreferencesStore.setState({ themeMode: "light", themePreview: null });
  const { unmount } = await renderHook(useTheme);
  expect(setTheme).toHaveBeenLastCalledWith("light");
  expect(setColorScheme).toHaveBeenLastCalledWith("light");
  await act(() => usePreferencesStore.setState({ themeMode: "dark" }));
  expect(setTheme).toHaveBeenLastCalledWith("dark");
  expect(setColorScheme).toHaveBeenLastCalledWith("dark");
  await act(() => usePreferencesStore.setState({ themeMode: "system" }));
  expect(setTheme).toHaveBeenLastCalledWith("system");
  // RN's Jest platform omits the version; Uniwind uses the legacy reset value.
  // The simulator regression verifies RN 0.87's real system-theme reset.
  expect(setColorScheme).toHaveBeenLastCalledWith(undefined);
  await unmount(); setTheme.mockRestore(); setColorScheme.mockRestore();
});

test("Android delegates native appearance to Uniwind without duplicate configuration updates", async () => {
  const platform = jest.replaceProperty(Platform, "OS", "android");
  const setTheme = jest.spyOn(Uniwind, "setTheme");
  const setColorScheme = jest.spyOn(Appearance, "setColorScheme");
  usePreferencesStore.setState({ themeMode: "light", themePreview: null });
  const { unmount } = await renderHook(useTheme);
  await act(() => usePreferencesStore.setState({ themeMode: "dark" }));
  expect(setTheme).toHaveBeenLastCalledWith("dark");
  expect(setColorScheme.mock.calls).toEqual([["light"], ["dark"]]);
  await unmount(); setTheme.mockRestore(); setColorScheme.mockRestore(); platform.restore();
});
