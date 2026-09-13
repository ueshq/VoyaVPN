import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useForm } from "react-hook-form";
import { describe, expect, it } from "vitest";

import { type ParsedProfileFormValues, type ProfileFormValues } from "./profile-form-schema";
import { createDefaultProfile } from "./profile-form-values";
import { SecurityPanel } from "./profile-security-panel";

function SecurityPanelHarness({ security }: { security: string }) {
  const form = useForm<ProfileFormValues, unknown, ParsedProfileFormValues>({
    // The schema input is a discriminated union, so the literal is built from the
    // same factory the editor uses and asserted once.
    defaultValues: {
      ...createDefaultProfile("vless"),
      remarks: "Pinned node",
    } as ProfileFormValues,
  });

  return (
    <>
      <output data-testid="cert">{String(form.watch("cert") ?? "")}</output>
      <SecurityPanel
        control={form.control}
        register={form.register}
        security={security}
      />
    </>
  );
}

describe("SecurityPanel", () => {
  it("shows no TLS fields for a plaintext node", () => {
    render(<SecurityPanelHarness security="" />);

    expect(screen.queryByText("SNI")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Pinned cert")).not.toBeInTheDocument();
  });

  it("edits TLS fields and keeps the pinned PEM in the form", async () => {
    const user = userEvent.setup();
    render(<SecurityPanelHarness security="tls" />);

    expect(screen.getByText("SNI")).toBeInTheDocument();
    expect(screen.getByText("ALPN")).toBeInTheDocument();
    expect(screen.queryByText("REALITY public key")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Fetch cert" })).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("Pinned cert"), "pem-body");
    expect(screen.getByTestId("cert")).toHaveTextContent("pem-body");
  });

  it("adds the REALITY key fields without retired options", () => {
    render(<SecurityPanelHarness security="reality" />);

    expect(screen.getByText("REALITY public key")).toBeInTheDocument();
    expect(screen.getByText("Short ID")).toBeInTheDocument();
    expect(screen.queryByText("Spider X")).not.toBeInTheDocument();
  });
});
