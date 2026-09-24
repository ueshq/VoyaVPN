import { act, renderHook } from "@testing-library/react-native";
import { Uniwind } from "uniwind";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { useTheme } from "./use-theme";

test("passes system preference through to Uniwind after explicit light and dark overrides", async () => {
  const setTheme = jest.spyOn(Uniwind, "setTheme");
  usePreferencesStore.setState({ themeMode: "light", themePreview: null });
  const { unmount } = await renderHook(useTheme);
  expect(setTheme).toHaveBeenLastCalledWith("light");
  await act(() => usePreferencesStore.setState({ themeMode: "dark" }));
  expect(setTheme).toHaveBeenLastCalledWith("dark");
  await act(() => usePreferencesStore.setState({ themeMode: "system" }));
  expect(setTheme).toHaveBeenLastCalledWith("system");
  await unmount(); setTheme.mockRestore();
});
