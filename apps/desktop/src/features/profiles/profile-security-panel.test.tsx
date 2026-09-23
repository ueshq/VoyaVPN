import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { createDefaultProfile } from "@voya/features/profiles/profile-form-values";
import {
  toEditorForm,
  type ProfileEditorForm,
} from "./profile-editor-form";
import { SecurityPanel } from "./profile-security-panel";

function SecurityPanelHarness({ security }: { security: string }) {
  const [form, setForm] = useState<ProfileEditorForm>(() =>
    toEditorForm({
      ...createDefaultProfile("vless"),
      remarks: "Pinned node",
    }),
  );

  return (
    <>
      <output data-testid="cert">{form.cert}</output>
      <SecurityPanel
        errors={{}}
        form={form}
        onFieldChange={(key, value) =>
          setForm((current) => ({ ...current, [key]: value }))
        }
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
