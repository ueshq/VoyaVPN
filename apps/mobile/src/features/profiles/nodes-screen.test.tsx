import { render, screen, userEvent, waitFor } from "@testing-library/react-native";
import type { QueryClient } from "@tanstack/react-query";
import type { MockBackend } from "@voya/client/mock-backend";
import { makePolicyGroupEntry } from "@voya/client/mock-seed";
import { useNodeListStore } from "@voya/client/node-list-store";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { setClipboard } from "@voya/client/platform";

import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { localeReady } from "~/native/platform-boot";
import { makeTestQueryClient, TestProviders } from "~/test/providers";

import { NodesScreen } from "./nodes-screen";

let activeQueryClient: QueryClient | null = null;

async function renderNodes() {
  const queryClient = makeTestQueryClient();
  activeQueryClient = queryClient;
  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <TestProviders queryClient={queryClient}>{children}</TestProviders>
  );

  return { queryClient, ...(await render(<NodesScreen />, { wrapper })) };
}

const readText = jest.fn<Promise<string>, []>();
const writeText = jest.fn<Promise<void>, [string]>();

function backend() {
  return voyaTransport() as MockBackend;
}

beforeAll(async () => {
  await localeReady;
});

beforeEach(() => {
  registerMobileBackend();
  // The screen reads the clipboard through the shared seam, so the test
  // replaces the seam rather than the native module behind it.
  readText.mockReset();
  writeText.mockReset().mockResolvedValue(undefined);
  setClipboard({ readText, writeText });
  useNodeListStore.setState(useNodeListStore.getInitialState());
  useRuntimeActionStore.setState(useRuntimeActionStore.getInitialState());
  useRuntimeEventStore.setState({ coreState: null });
});

afterEach(() => {
  activeQueryClient?.clear();
  activeQueryClient = null;
});

describe("NodesScreen", () => {
  it("groups nodes by source, local last, with each group's size", async () => {
    await renderNodes();

    // Twice on purpose: the group header over its nodes, and the row in the
    // subscriptions section that refreshes it.
    expect(await screen.findAllByText("Example provider")).toHaveLength(2);
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

  it("filters the list by name and says so when nothing matches", async () => {
    await renderNodes();
    const user = userEvent.setup();
    const search = await screen.findByPlaceholderText(
      "Search node name, address or subscription",
    );

    await user.type(search, "Tokyo");
    expect(await screen.findByText("🇯🇵 Tokyo")).toBeOnTheScreen();
    expect(screen.queryByText("🇸🇬 Singapore")).not.toBeOnTheScreen();

    await user.clear(search);
    await user.type(search, "Reykjavik");
    expect(await screen.findByText("No matching nodes")).toBeOnTheScreen();
  });

  it("imports a share link pasted on the clipboard", async () => {
    readText.mockResolvedValue("vless://token@example.test:443#Osaka");
    await renderNodes();
    const user = userEvent.setup();

    await user.press(await screen.findByText("Import from clipboard"));

    // The import, the invalidation it triggers and the refetches behind it all
    // have to settle before the new node can appear.
    expect(await screen.findByText("Osaka", {}, { timeout: 5000 })).toBeOnTheScreen();
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

  it("refreshes a subscription without leaving the list", async () => {
    await renderNodes();
    const user = userEvent.setup();

    await user.press(await screen.findByLabelText("Update subscription Example provider"));

    await waitFor(() =>
      expect(backend().state.calls.map((call) => call.command)).toContain("updateSubscriptions"),
    );
  });

  it("uses a policy group instead of the selected node", async () => {
    const backendInstance = backend();
    backendInstance.state.policyGroups = [
      makePolicyGroupEntry(0, { name: "Fastest" }),
    ];
    await renderNodes();
    const user = userEvent.setup();

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

    await user.press(await screen.findByText("Share links"));

    // The link the backend exported, handed straight to the system clipboard.
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("#🇯🇵 Tokyo"));
  });

  it("shows a node as a QR code rendered by the backend", async () => {
    await renderNodes();
    const user = userEvent.setup();

    await user.longPress(await screen.findByText("🇯🇵 Tokyo"));
    await user.press(await screen.findByText("Show QR"));

    expect(await screen.findByLabelText("Generated QR code")).toBeOnTheScreen();
    expect(backend().state.calls.map((call) => call.command)).toContain("generateQrCode");
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

  it("deletes a local node from the sheet", async () => {
    await renderNodes();
    const user = userEvent.setup();

    await user.longPress(await screen.findByText("🇸🇬 Singapore"));
    await user.press(await screen.findByText("Delete"));

    await waitFor(() =>
      expect(backend().state.profiles.map((entry) => entry.profile.id)).not.toContain("profile-1"),
    );
  });

  it("reports a clipboard that holds nothing importable", async () => {
    readText.mockResolvedValue("   ");
    await renderNodes();
    const user = userEvent.setup();

    await user.press(await screen.findByText("Import from clipboard"));

    expect(await screen.findByText("Clipboard is empty.")).toBeOnTheScreen();
  });
});
