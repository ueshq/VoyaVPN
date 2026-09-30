import { screen, userEvent } from "@testing-library/react-native";
import { Linking } from "react-native";

import * as native from "~/native/device-actions";
import { localeReady } from "~/native/platform-boot";
import { renderScreen } from "~/test/providers";

import { AboutScreen } from "./about-screen";
import { PRIVACY_POLICY_URL, SUPPORT_URL } from "./privacy-notice";

type DeviceActions = ReturnType<typeof native.deviceActions>;

beforeAll(async () => { await localeReady; });
beforeEach(() => {
  jest.spyOn(native, "deviceActions").mockReturnValue({ appVersion: async () => "0.1.0 (412)" } as DeviceActions);
});
afterEach(() => jest.restoreAllMocks());

test("shows the version and the same data statements as the first-run notice", async () => {
  await renderScreen(<AboutScreen />);

  expect(await screen.findByText("VoyaVPN 0.1.0 (412)")).toBeOnTheScreen();
  expect(screen.getByText("Privacy information")).toBeOnTheScreen();
  expect(screen.getByText(/It collects no data/)).toBeOnTheScreen();
  expect(screen.getByText(/stay on this device/)).toBeOnTheScreen();
  expect(screen.getByText(/only when you share them yourself/)).toBeOnTheScreen();
});

test("links to the privacy policy and to support", async () => {
  const openURL = jest.spyOn(Linking, "openURL").mockResolvedValue(undefined);
  await renderScreen(<AboutScreen />);
  const user = userEvent.setup();

  await user.press(screen.getByTestId("privacy-policy-link"));
  await user.press(screen.getByTestId("support-link"));

  expect(openURL.mock.calls).toEqual([[PRIVACY_POLICY_URL], [SUPPORT_URL]]);
});
