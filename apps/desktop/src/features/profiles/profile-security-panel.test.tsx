import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useForm } from "react-hook-form";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { type ParsedProfileFormValues, type ProfileFormValues } from "./profile-form-schema";
import { createDefaultProfile } from "./profile-form-values";
import { SecurityPanel } from "./profile-security-panel";

const ipcMocks = vi.hoisted(() => ({
  calculateCertificateSha256: vi.fn(),
  fetchCertificate: vi.fn(),
}));

vi.mock("@/ipc/commands", () => ipcMocks);

function SecurityPanelHarness({ address, port }: { address: string; port: number }) {
  const form = useForm<ProfileFormValues, unknown, ParsedProfileFormValues>({
    // The schema input is a discriminated union, so the literal is built from the
    // same factory the editor uses and asserted once.
    defaultValues: {
      ...createDefaultProfile("vless"),
      address,
      port,
      remarks: "Pinned node",
      sni: "",
    } as ProfileFormValues,
  });

  return (
    <>
      <output data-testid="cert">{String(form.watch("cert") ?? "")}</output>
      <output data-testid="cert-sha">{String(form.watch("certSha") ?? "")}</output>
      <SecurityPanel
        control={form.control}
        getValues={form.getValues}
        register={form.register}
        security="tls"
        setValue={form.setValue}
      />
    </>
  );
}

function renderPanel({ address = "node.example.test", port = 443 } = {}) {
  return render(<SecurityPanelHarness address={address} port={port} />);
}

beforeEach(() => {
  Object.values(ipcMocks).forEach((mock) => mock.mockReset());
  ipcMocks.fetchCertificate.mockResolvedValue({
    chainCount: 2,
    pem: "-----BEGIN CERTIFICATE-----",
    sha256: ["sha-a", "sha-b"],
    warning: null,
  });
  ipcMocks.calculateCertificateSha256.mockResolvedValue(["sha-local"]);
});

describe("SecurityPanel certificate fetch", () => {
  it("refuses to fetch without an address and port", async () => {
    const user = userEvent.setup();
    renderPanel({ address: "", port: 0 });

    await user.click(screen.getByRole("button", { name: "Fetch cert" }));

    expect(
      await screen.findByText("Address and port are required before fetching a certificate."),
    ).toBeInTheDocument();
    expect(ipcMocks.fetchCertificate).not.toHaveBeenCalled();
  });

  it("forwards the endpoint, the chain flag and the allow-insecure toggle verbatim", async () => {
    const user = userEvent.setup();
    renderPanel();

    await user.click(screen.getByRole("button", { name: "Fetch cert" }));
    await waitFor(() =>
      expect(ipcMocks.fetchCertificate).toHaveBeenCalledWith({
        address: "node.example.test",
        allowInsecure: false,
        includeChain: false,
        port: 443,
        serverName: "node.example.test",
      }),
    );

    // An inverted `allowInsecure` would silently disable certificate checks.
    await user.click(screen.getByRole("switch", { name: "Allow insecure fetch" }));
    await user.click(screen.getByRole("button", { name: "Fetch chain" }));

    await waitFor(() =>
      expect(ipcMocks.fetchCertificate).toHaveBeenLastCalledWith(
        expect.objectContaining({ allowInsecure: true, includeChain: true }),
      ),
    );
  });

  it("writes the fetched PEM and joined fingerprints back into the form", async () => {
    const user = userEvent.setup();
    renderPanel();

    await user.click(screen.getByRole("button", { name: "Fetch cert" }));

    await waitFor(() => expect(screen.getByTestId("cert")).toHaveTextContent("BEGIN CERTIFICATE"));
    expect(screen.getByTestId("cert-sha")).toHaveTextContent("sha-a,sha-b");
    expect(await screen.findByText("Fetched 2 certificate(s).")).toBeInTheDocument();
  });

  it("surfaces the backend warning instead of the success count", async () => {
    const user = userEvent.setup();
    ipcMocks.fetchCertificate.mockResolvedValue({
      chainCount: 1,
      pem: "-----BEGIN CERTIFICATE-----",
      sha256: ["sha-a"],
      warning: "The certificate chain is self-signed.",
    });
    renderPanel();

    await user.click(screen.getByRole("button", { name: "Fetch cert" }));

    expect(await screen.findByText("The certificate chain is self-signed.")).toBeInTheDocument();
    expect(screen.queryByText("Fetched 1 certificate(s).")).not.toBeInTheDocument();
  });

  it("reports a rejected fetch without clearing the form", async () => {
    const user = userEvent.setup();
    ipcMocks.fetchCertificate.mockRejectedValue(new Error("tls handshake failed"));
    renderPanel();

    await user.click(screen.getByRole("button", { name: "Fetch cert" }));

    expect(await screen.findByText("tls handshake failed")).toBeInTheDocument();
    expect(screen.getByTestId("cert")).toBeEmptyDOMElement();
  });
});

describe("SecurityPanel SHA-256 calculation", () => {
  it("refuses to hash an empty PEM field", async () => {
    const user = userEvent.setup();
    renderPanel();

    await user.click(screen.getByRole("button", { name: "Calculate SHA" }));

    expect(
      await screen.findByText("Paste or fetch a PEM certificate before calculating SHA-256."),
    ).toBeInTheDocument();
    expect(ipcMocks.calculateCertificateSha256).not.toHaveBeenCalled();
  });

  it("hashes the pasted PEM and stores the fingerprints", async () => {
    const user = userEvent.setup();
    renderPanel();

    await user.type(screen.getByLabelText("Pinned cert"), "pem-body");
    await user.click(screen.getByRole("button", { name: "Calculate SHA" }));

    await waitFor(() =>
      expect(ipcMocks.calculateCertificateSha256).toHaveBeenCalledWith("pem-body"),
    );
    expect(screen.getByTestId("cert-sha")).toHaveTextContent("sha-local");
    expect(await screen.findByText("Calculated 1 SHA-256 fingerprint(s).")).toBeInTheDocument();
  });
});
