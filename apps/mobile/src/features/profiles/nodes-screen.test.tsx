import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, userEvent } from "@testing-library/react-native";
import type { MockBackend } from "@voya/client/mock-backend";
import { useNodeListStore } from "@voya/client/node-list-store";
import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { setClipboard } from "@voya/client/platform";
import type { ReactNode } from "react";

import { registerMobileBackend, voyaTransport } from "~/ipc/platform";
import { localeReady } from "~/native/platform-boot";

import { NodesScreen } from "./nodes-screen";

let activeQueryClient: QueryClient | null = null;

async function renderNodes() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  activeQueryClient = queryClient;
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );

  return { queryClient, ...(await render(<NodesScreen />, { wrapper })) };
}

const readText = jest.fn<Promise<string>, []>();

beforeAll(async () => {
  await localeReady;
});

beforeEach(() => {
  registerMobileBackend();
  // The screen reads the clipboard through the shared seam, so the test
  // replaces the seam rather than the native module behind it.
  readText.mockReset();
  setClipboard({ readText, writeText: jest.fn() });
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

    expect(await screen.findByText("Example provider")).toBeOnTheScreen();
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

  it("reports a clipboard that holds nothing importable", async () => {
    readText.mockResolvedValue("   ");
    await renderNodes();
    const user = userEvent.setup();

    await user.press(await screen.findByText("Import from clipboard"));

    expect(await screen.findByText("Clipboard is empty.")).toBeOnTheScreen();
  });
});
