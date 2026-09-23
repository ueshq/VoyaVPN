import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderWithQuery } from "@/test/render";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { changeLocale } from "@voya/i18n";
import type { DnsSettings } from "@voya/contracts";
import {
  deferred,
  installSettingsBackend,
  serverSettings,
  settingsIpc,
} from "@voya/features/settings/settings-backend.test-fixture";
import { DnsPane } from "./dns-pane";
import { useDnsSettings } from "@voya/features/dns/use-dns-settings";

beforeEach(async () => { installSettingsBackend(); await changeLocale("en"); });
afterEach(cleanup);
function mount() {
  function Pane() { return <DnsPane controller={useDnsSettings()} />; }
  return renderWithQuery(<Pane />);
}
/** Puts `dns` in the fake backend before the pane loads it. */
async function stored(dns: Partial<DnsSettings>) {
  await settingsIpc.saveDnsSettings({ ...serverSettings().dns, ...dns });
  settingsIpc.saveDnsSettings.mockClear();
}
function lastSaved() {
  return settingsIpc.saveDnsSettings.mock.lastCall?.[0] as DnsSettings | undefined;
}
async function commit(label: string, value: string) {
  const input = await screen.findByLabelText(label);
  fireEvent.change(input, { target: { value } });
  fireEvent.blur(input);
  return input;
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

  it("shows a loading line until the settings arrive", async () => {
    const load = deferred<DnsSettings>();
    settingsIpc.loadDnsSettings.mockReturnValueOnce(load.promise);
    mount();
    expect(screen.getByText("Loading DNS settings")).toBeInTheDocument();
    expect(screen.queryByLabelText("Remote DNS")).not.toBeInTheDocument();

    await act(async () => load.resolve(structuredClone(serverSettings().dns)));
    expect(await screen.findByLabelText("Remote DNS")).toBeInTheDocument();
    expect(screen.queryByText("Loading DNS settings")).not.toBeInTheDocument();
  });

  it("fills a resolver from a preset and marks the preset in use", async () => {
    // Whitespace around a typed address still matches its preset.
    await stored({ direct: " 223.5.5.5 " });
    mount();
    const direct = await screen.findByRole("group", { name: "Common direct DNS" });
    expect(within(direct).getByRole("button", { name: "Alibaba Cloud" })).toHaveAttribute("aria-pressed", "true");
    expect(within(direct).getByRole("button", { name: "DNSPod (Tencent)" })).toHaveAttribute("aria-pressed", "false");

    const remote = screen.getByRole("group", { name: "Common remote DNS" });
    fireEvent.click(within(remote).getByRole("button", { name: "Cloudflare" }));

    await waitFor(() => expect(lastSaved()?.remote).toBe("https://cloudflare-dns.com/dns-query"));
    expect(screen.getByLabelText("Remote DNS")).toHaveValue("https://cloudflare-dns.com/dns-query");
    expect(within(remote).getByRole("button", { name: "Cloudflare" })).toHaveAttribute("aria-pressed", "true");
    expect(within(remote).getByRole("button", { name: "Google" })).toHaveAttribute("aria-pressed", "false");

    fireEvent.click(within(direct).getByRole("button", { name: "DNSPod (Tencent)" }));
    await waitFor(() => expect(lastSaved()?.direct).toBe("119.29.29.29"));
    expect(screen.getByLabelText("Direct DNS")).toHaveValue("119.29.29.29");
  });

  it("commits the direct and bootstrap resolvers typed by hand", async () => {
    mount();
    await commit("Direct DNS", "10.0.0.53");
    await waitFor(() => expect(lastSaved()?.direct).toBe("10.0.0.53"));
    await commit("Bootstrap DNS", "223.6.6.6");
    await waitFor(() => expect(lastSaved()).toMatchObject({ bootstrap: "223.6.6.6", direct: "10.0.0.53" }));
  });

  it("shows strategies that generate nothing as Default and saves Default as no strategy", async () => {
    const user = userEvent.setup();
    await stored({ directStrategy: "AsIs", proxyStrategy: "ForceIPv6" });
    mount();
    const directStrategy = await screen.findByRole("combobox", { name: "Direct strategy" });
    const proxyStrategy = screen.getByRole("combobox", { name: "Proxy strategy" });
    expect(directStrategy).toHaveTextContent("Default");
    expect(proxyStrategy).toHaveTextContent("IPv6 only");

    await user.click(directStrategy);
    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    // "AsIs" and "UseIP" are the default under other names, so only one is offered.
    expect(options.map((option) => option.textContent)).toEqual([
      "Default", "Prefer IPv4", "Prefer IPv6", "IPv4 only", "IPv6 only",
    ]);
    await user.click(screen.getByRole("option", { name: "IPv4 only" }));
    await waitFor(() => expect(lastSaved()?.directStrategy).toBe("ForceIPv4"));

    await user.click(proxyStrategy);
    await user.click(await screen.findByRole("option", { name: "Default" }));
    await waitFor(() => expect(lastSaved()?.proxyStrategy).toBeNull());
    expect(serverSettings().dns).toMatchObject({ directStrategy: "ForceIPv4", proxyStrategy: null });
  });

  it("shows Global FakeIP off while FakeIP is off and brings the stored choice back with it", async () => {
    await stored({ fakeIp: false, globalFakeIp: true });
    mount();
    const globalFakeIp = await screen.findByLabelText("Global FakeIP");
    expect(globalFakeIp).not.toBeChecked();
    expect(globalFakeIp).toBeDisabled();

    fireEvent.click(screen.getByLabelText("FakeIP"));

    await waitFor(() => expect(lastSaved()).toMatchObject({ fakeIp: true, globalFakeIp: true }));
    expect(screen.getByLabelText("Global FakeIP")).toBeChecked();
    expect(screen.getByLabelText("Global FakeIP")).toBeEnabled();

    fireEvent.click(screen.getByLabelText("Global FakeIP"));
    await waitFor(() => expect(lastSaved()).toMatchObject({ fakeIp: true, globalFakeIp: false }));
    expect(screen.getByLabelText("Global FakeIP")).not.toBeChecked();
  });

  it("saves the other behaviour switches as soon as they flip", async () => {
    mount();
    fireEvent.click(await screen.findByLabelText("Common hosts"));
    await waitFor(() => expect(lastSaved()?.addCommonHosts).toBe(true));
    fireEvent.click(screen.getByLabelText("Block HTTPS/SVCB"));
    await waitFor(() => expect(lastSaved()).toMatchObject({ addCommonHosts: true, blockBindingQuery: true }));
  });

  it("counts invalid fields, opens More settings for them, and clears both once fixed", async () => {
    mount();
    const advanced = (await screen.findByText("More settings")).closest("details");
    expect(advanced).not.toHaveAttribute("open");
    expect(screen.queryByText(/errors$/)).not.toBeInTheDocument();

    const expected = await commit("Expected IPs", "10.0.0.0/8, 1.1.1.1 2.2.2.2");
    await waitFor(() => expect(expected).toHaveAccessibleDescription(
      "For direct DNS: use these IP ranges to check whether a response should be accepted. Expected IPs must be comma-separated without embedded whitespace",
    ));
    expect(screen.getByText("1 errors")).toBeInTheDocument();
    expect(advanced).toHaveAttribute("open");
    await commit("Hosts", "missing-answer");
    expect(await screen.findByText("2 errors")).toBeInTheDocument();
    // Invalid values never reach the backend.
    expect(settingsIpc.saveDnsSettings).not.toHaveBeenCalled();

    await commit("Expected IPs", "10.0.0.0/8, 1.1.1.1");
    await commit("Hosts", "# local overrides\n\nexample.test 127.0.0.1");
    await waitFor(() => expect(screen.queryByText(/errors$/)).not.toBeInTheDocument());
    expect(serverSettings().dns).toMatchObject({
      directExpectedIps: "10.0.0.0/8, 1.1.1.1",
      hosts: "# local overrides\n\nexample.test 127.0.0.1",
    });
  });
});
