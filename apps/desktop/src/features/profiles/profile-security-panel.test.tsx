import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import {
  createDefaultDraft,
  type ProfileDraft,
} from "@voya/features/profiles/profile-draft";
import { SecurityPanel } from "./profile-security-panel";

function SecurityPanelHarness({ tlsMode }: { tlsMode: ProfileDraft["tlsMode"] }) {
  const [draft, setDraft] = useState<ProfileDraft>(() =>
    ({ ...createDefaultDraft("vless"), remarks: "Pinned node", tlsMode }),
  );

  return (
    <>
      <output data-testid="cert">{draft.certificatePem}</output>
      <output data-testid="form">{JSON.stringify(draft)}</output>
      <SecurityPanel
        draft={draft}
        errors={{ certificatePem: "Invalid certificate" }}
        onChange={(key, value) =>
          setDraft((current) => ({ ...current, [key]: value }))
        }
      />
    </>
  );
}

describe("SecurityPanel", () => {
  it("shows no TLS fields for a plaintext node", () => {
    render(<SecurityPanelHarness tlsMode="none" />);

    expect(screen.queryByText("SNI")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Pinned cert")).not.toBeInTheDocument();
  });

  it("edits TLS fields and keeps the pinned PEM in the form", async () => {
    const user = userEvent.setup();
    render(<SecurityPanelHarness tlsMode="tls" />);

    expect(screen.getByText("SNI")).toBeInTheDocument();
    expect(screen.getByText("ALPN")).toBeInTheDocument();
    expect(screen.queryByText("REALITY public key")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Fetch cert" })).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("Pinned cert"), "pem-body");
    expect(screen.getByTestId("cert")).toHaveTextContent("pem-body");
  });

  it("adds the REALITY key fields without retired options", () => {
    render(<SecurityPanelHarness tlsMode="reality" />);

    expect(screen.getByText("REALITY public key")).toBeInTheDocument();
    expect(screen.getByText("Short ID")).toBeInTheDocument();
    expect(screen.queryByText("Spider X")).not.toBeInTheDocument();
  });
});

it("changes security mode and persists the editable TLS and REALITY fields", async () => {
  const user = userEvent.setup();
  render(<SecurityPanelHarness tlsMode="tls" />);
  for (const [label, value, key] of [
    ["SNI", "example.test", "serverName"], ["ALPN", "h2", "alpn"],
  ]) {
    await user.type(screen.getByLabelText(label), value);
    expect(JSON.parse(screen.getByTestId("form").textContent ?? "{}")[key]).toBe(value);
  }
  await user.click(screen.getByText("More settings"));
  await user.type(screen.getByLabelText("ECH config list"), "ech-value");
  expect(JSON.parse(screen.getByTestId("form").textContent ?? "{}").echConfig).toBe("ech-value");
  expect(screen.getByText("Invalid certificate")).toBeInTheDocument();
  await user.click(screen.getByRole("combobox"));
  await user.click(screen.getByRole("option", { name: "REALITY" }));
  for (const [label, value, key] of [
    ["REALITY public key", "public-key", "realityPublicKey"], ["Short ID", "abcd", "realityShortId"],
  ]) {
    await user.type(screen.getByLabelText(label), value);
    expect(JSON.parse(screen.getByTestId("form").textContent ?? "{}")[key]).toBe(value);
  }
});
