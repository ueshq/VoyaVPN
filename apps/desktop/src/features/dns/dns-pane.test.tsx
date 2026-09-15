import { cleanup, fireEvent, screen, waitFor } from "@testing-library/react";
import { renderWithQuery } from "@/test/render";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { changeLocale } from "@voya/i18n";
import { resetSettingsBackend, settingsIpc } from "@/features/settings/settings-backend.test-fixture";
import { DnsPane } from "./dns-pane";
import { useDnsSettings } from "./use-dns-settings";

vi.mock("@/ipc/commands", async () => (await import("@/features/settings/settings-backend.test-fixture")).settingsIpc);
beforeEach(async () => { resetSettingsBackend(); await changeLocale("en"); });
afterEach(cleanup);
function mount() {
  function Pane() { return <DnsPane controller={useDnsSettings()} />; }
  return renderWithQuery(<Pane />);
}

describe("DNS fields", () => {
  it("commits a resolver on blur and has no manual save controls", async () => {
    mount();
    const input = await screen.findByLabelText("Remote DNS");
    fireEvent.change(input, { target: { value: "https://dns.google/dns-query" } });
    expect(settingsIpc.saveDnsSettings).not.toHaveBeenCalled();
    fireEvent.blur(input);
    await waitFor(() => expect(settingsIpc.saveDnsSettings).toHaveBeenCalledWith(expect.objectContaining({ remote: "https://dns.google/dns-query" })));
    expect(screen.queryByRole("button", { name: /Save|Reload/ })).not.toBeInTheDocument();
  });

  it("keeps multiline input until blur and shows validation at its field", async () => {
    mount();
    const input = await screen.findByLabelText("Hosts");
    fireEvent.change(input, { target: { value: "invalid-host" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(settingsIpc.saveDnsSettings).not.toHaveBeenCalled();
    fireEvent.blur(input);
    await waitFor(() => expect(input).toHaveAttribute("aria-invalid", "true"));
    expect(input).toHaveAccessibleDescription("Every host line must contain a domain and at least one answer");
    expect(input).toHaveValue("invalid-host");
  });

  it("saves checkboxes immediately and preserves the FakeIP dependency", async () => {
    mount();
    const fakeIp = await screen.findByLabelText("FakeIP");
    expect(screen.getByLabelText("Global FakeIP")).toBeDisabled();
    fireEvent.click(fakeIp);
    await waitFor(() => expect(settingsIpc.saveDnsSettings).toHaveBeenCalledWith(expect.objectContaining({ fakeIp: true })));
    expect(screen.getByLabelText("Global FakeIP")).toBeEnabled();
  });
});
