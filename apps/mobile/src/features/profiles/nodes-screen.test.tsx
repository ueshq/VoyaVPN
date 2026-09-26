import { act } from "@testing-library/react-native";
import { Alert, View, type MeasureOnSuccessCallback } from "react-native";
import { fireEvent, screen, userEvent, waitFor } from "@testing-library/react-native";
import type { MockBackend } from "@voya/client/mock-backend";
import { makePolicyGroupEntry } from "@voya/client/mock-seed";
import { useNodeListStore } from "@voya/client/node-list-store";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { setClipboard } from "@voya/client/platform";

import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { mockBackend, mockTransport } from "~/test/mock-transport";
import { localeReady } from "~/native/platform-boot";
import { renderScreen } from "~/test/providers";

import { NodesScreen } from "./nodes-screen";

function renderNodes() {
  return renderScreen(<NodesScreen />);
}

const readText = jest.fn<Promise<string>, []>();
const writeText = jest.fn<Promise<void>, [string]>();

beforeAll(async () => {
  await localeReady;
});

beforeEach(() => {
  registerMobileBackend(mockTransport());
  // The screen reads the clipboard through the shared seam, so the test
  // replaces the seam rather than the native module behind it.
  readText.mockReset();
  writeText.mockReset().mockResolvedValue(undefined);
  setClipboard({ readText, writeText });
  useNodeListStore.setState(useNodeListStore.getInitialState());
  useRuntimeActionStore.setState(useRuntimeActionStore.getInitialState());
  useRuntimeEventStore.setState({ coreState: null });
});


describe("NodesScreen", () => {
  it("groups nodes by source, local last, with each group's size", async () => {
    await renderNodes();

    // Twice on purpose: the group header over its nodes, and the row in the
    // subscriptions section that refreshes it.
    expect(await screen.findAllByText("Example provider")).toHaveLength(1);
    expect(screen.getByText("Local nodes")).toBeOnTheScreen();
    expect(screen.getByText("🇯🇵 Tokyo")).toBeOnTheScreen();
    expect(screen.getByText("🇺🇸 Los Angeles")).toBeOnTheScreen();
    expect(screen.getAllByText("1 nodes").length).toBeGreaterThan(0);
  });

  it("marks the node in use and switches to another on tap", async () => {
    await renderNodes();
    const user = userEvent.setup();

    expect(await screen.findByText("Selected")).toBeOnTheScreen();

    await user.press(screen.getByText("🇸🇬 Singapore"));

    const backend = voyaTransport() as MockBackend;
    expect(backend.state.calls.map((call) => call.command)).toContain("setActiveProfile");
    expect(backend.state.profiles.find((entry) => entry.isActive)?.profile.id).toBe("profile-1");
  });

  it("does not read the clipboard merely by opening the list", async () => {
    await renderNodes();
    await screen.findByLabelText("Add nodes or subscription");
    expect(readText).not.toHaveBeenCalled();
  });

  it("measures every listed node and fills the latencies back in", async () => {
    await renderNodes();
    const user = userEvent.setup();
    // The run tests what the list is showing, so there has to be a list.
    await screen.findByText("🇯🇵 Tokyo");

    await user.press(screen.getByText("Test all"));

    const backend = voyaTransport() as MockBackend;
    const run = backend.state.calls.find((call) => call.command === "runSpeedtest");
    const target = (run?.args[0] as { target: { profileIds: string[]; scope: string } }).target;
    expect(target.scope).toBe("profiles");
    expect([...target.profileIds].sort()).toEqual(["profile-0", "profile-1", "profile-2"]);
    // The measurement lands on the row it belongs to, off the streamed results
    // rather than a refetch.
    expect(await screen.findByText("40 ms")).toBeOnTheScreen();
  });

  it("orders nodes by latency from the sort menu", async () => {
    await renderNodes();
    const user = userEvent.setup();
    await screen.findByText("🇯🇵 Tokyo");

    const trigger = screen.getByLabelText("Sort order");
    expect(trigger).toHaveAccessibilityValue({ text: "Default order" });

    // The popover positions itself off two native layouts a test renderer
    // never produces: it mounts once its trigger measures, and takes touches
    // once its own content has laid out below the top of the screen. Fixed
    // frames stand in for both.
    const measure = jest
      .spyOn(View.prototype as { measure: (callback: MeasureOnSuccessCallback) => void }, "measure")
      .mockImplementation((callback) => callback(0, 0, 48, 48, 280, 80));
    try {
      await user.press(trigger);
      await fireEvent(await screen.findByText("Sort order"), "layout", {
        nativeEvent: { layout: { x: 108, y: 137, width: 220, height: 120 } },
      });
      await user.press(screen.getByText("Lowest latency"));
    } finally {
      measure.mockRestore();
    }

    expect(useNodeListStore.getState().sortByLatency).toBe(true);
    await waitFor(() =>
      expect(screen.getByLabelText("Sort order")).toHaveAccessibilityValue({ text: "Lowest latency" }),
    );
  });

  it("uses a policy group instead of the selected node", async () => {
    const backendInstance = mockBackend();
    backendInstance.state.policyGroups = [
      makePolicyGroupEntry(0, { name: "Fastest" }),
    ];
    await renderNodes();
    const user = userEvent.setup();

    await user.press(await screen.findByText("Policy groups"));
    await user.press(await screen.findByRole("button", { name: /Fastest/ }));

    await waitFor(() =>
      expect(backendInstance.state.calls.map((call) => call.command)).toContain(
        "setActivePolicyGroup",
      ),
    );
  });

  it("offers share, QR and delete behind a long press", async () => {
    await renderNodes();
    const user = userEvent.setup();

    await user.longPress(await screen.findByText("🇯🇵 Tokyo"));

    await user.press(await screen.findByText("Copy link"));

    // The link the backend exported, handed straight to the system clipboard.
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("#🇯🇵 Tokyo"));
  });

  it("shows a node as a QR code rendered by the backend", async () => {
    await renderNodes();
    const user = userEvent.setup();

    await user.longPress(await screen.findByText("🇯🇵 Tokyo"));
    await user.press(await screen.findByText("Show QR"));

    expect(await screen.findByLabelText("Generated QR code")).toBeOnTheScreen();
    expect(mockBackend().state.calls.map((call) => call.command)).toContain("generateQrCode");
  });

  it("will not offer to delete a node the subscription owns", async () => {
    await renderNodes();
    const user = userEvent.setup();

    await user.longPress(await screen.findByText("🇺🇸 Los Angeles"));

    expect(
      await screen.findByText(
        "Subscription nodes come from their subscription and cannot be edited, moved or deleted",
      ),
    ).toBeOnTheScreen();
    expect(screen.queryByText("Delete")).toBeNull();
  });

  it("deletes a local node only after confirmation", async () => {
    const alert = jest.spyOn(Alert, "alert").mockImplementation(() => {});
    await renderNodes();
    const user = userEvent.setup();

    await user.longPress(await screen.findByText("🇸🇬 Singapore"));
    await user.press(await screen.findByText("Delete"));
    expect(mockBackend().state.profiles.map((entry) => entry.profile.id)).toContain("profile-1");
    const confirm = alert.mock.calls.at(-1)?.[2]?.find((button) => button.style === "destructive");
    await act(() => confirm?.onPress?.());
    alert.mockRestore();

    await waitFor(() =>
      expect(mockBackend().state.profiles.map((entry) => entry.profile.id)).not.toContain("profile-1"),
    );
  });

});
