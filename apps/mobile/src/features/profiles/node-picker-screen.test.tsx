import { screen, userEvent, waitFor } from "@testing-library/react-native";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useConnectionPreferences } from "@voya/client/connection-preferences";
import { registerMobileBackend } from "~/ipc/platform";
import { mockBackend, mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { renderScreen } from "~/test/providers";
import * as navigation from "~/app/navigation";
import { NodePickerScreen } from "./node-picker-screen";

beforeAll(async () => { await localeReady; });
beforeEach(() => {
  registerMobileBackend(mockTransport());
  useRuntimeEventStore.setState(useRuntimeEventStore.getInitialState());
  useRuntimeActionStore.setState(useRuntimeActionStore.getInitialState());
  useConnectionPreferences.setState({ autoConnect: false, recentIds: [] });
});
afterEach(() => jest.restoreAllMocks());
it("chooses without connecting while stopped and offers no management actions", async () => {
  const backend = mockBackend();
  backend.state.profiles.forEach((entry) => { entry.isActive = false; });
  const navigate = jest.spyOn(navigation, "navigateToTab").mockImplementation(() => {});
  await renderScreen(<NodePickerScreen />);
  const row = await screen.findByText("🇯🇵 Tokyo");
  await userEvent.setup().press(row);
  await waitFor(() => expect(navigate).toHaveBeenCalledWith("home"));
  expect(backend.state.calls.some((call) => call.command === "setActiveProfile")).toBe(true);
  expect(backend.state.calls.some((call) => call.command === "connectActiveProfile")).toBe(false);
  expect(screen.queryByText("Test all")).toBeNull();
  expect(screen.queryByText("Add nodes or subscription")).toBeNull();
});
it("searches without testing nodes and filters deleted recent ids", async () => {
  useConnectionPreferences.setState({ recentIds: ["deleted"] });
  await renderScreen(<NodePickerScreen />);
  await screen.findByText("🇯🇵 Tokyo");
  await userEvent.setup().type(screen.getByPlaceholderText("Search nodes"), "not-a-node");
  expect(await screen.findByText("No matching nodes")).toBeOnTheScreen();
  expect(mockBackend().state.calls.some((call) => call.command === "runSpeedtest")).toBe(false);
});
it("keeps a failed live switch on screen and reports it only inline", async () => {
  const backend = mockBackend();
  const next = backend.state.profiles.find((entry) => !entry.isActive)!;
  backend.state.runtime = { ...backend.state.runtime, state: "connected", activeProfileId: backend.state.profiles.find((entry) => entry.isActive)!.profile.id };
  useRuntimeEventStore.setState({ coreState: backend.state.runtime });
  const restart = jest.spyOn(backend.commands, "restartCore").mockRejectedValue(new Error("could not switch"));
  const navigate = jest.spyOn(navigation, "navigateToTab").mockImplementation(() => {});
  await renderScreen(<NodePickerScreen />);
  await userEvent.setup().press(await screen.findByText(next.profile.remarks));
  expect(await screen.findByText("Could not switch. Check the current connection on Home.")).toBeOnTheScreen();
  expect(restart).toHaveBeenCalledTimes(1);
  expect(navigate).not.toHaveBeenCalled();
  expect(useRuntimeActionStore.getState().switchingId).toBeNull();
});
