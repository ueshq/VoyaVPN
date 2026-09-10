import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Input } from "@voya/ui/components/input";
import { NumberField, SettingsCheckbox, SettingsFields, SettingsGroup, SettingsRow } from "./settings-form";
import { useToastStore } from "@/stores/toast-store";

describe("settings form", () => {
  it("associates row labels and groups with accessible controls", () => {
    render(<SettingsGroup title="Startup"><SettingsRow htmlFor="field" label="Name"><Input id="field" /></SettingsRow>
      <SettingsFields errors={{ autostart: "Not allowed" }}><SettingsCheckbox field="autostart" checked label="Autostart" onCheckedChange={vi.fn()} /></SettingsFields>
    </SettingsGroup>);
    expect(screen.getByRole("region", { name: "Startup" })).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveAttribute("id", "field");
    expect(screen.getByLabelText("Autostart")).toHaveAccessibleDescription("Not allowed");
  });

  it("retains invalid numeric text and only commits valid whole numbers", () => {
    const onChange = vi.fn();
    render(<NumberField field="mtu" label="MTU" value={1500} onChange={onChange} />);
    const input = screen.getByLabelText("MTU");
    fireEvent.change(input, { target: { value: "" } });
    expect(input).toHaveValue("");
    fireEvent.blur(input);
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: "1.5" } });
    fireEvent.blur(input);
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: "9000" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith(9000);
    fireEvent.blur(input);
    expect(onChange).toHaveBeenCalledTimes(1);
  });

  it("clears nullable numbers and reports invalid numbers on leaving", () => {
    const onChange = vi.fn();
    const { unmount } = render(<NumberField field="pageSize" label="Batch size" nullable value={10} onChange={onChange} />);
    fireEvent.change(screen.getByLabelText("Batch size"), { target: { value: "" } });
    fireEvent.blur(screen.getByLabelText("Batch size"));
    expect(onChange).toHaveBeenCalledWith(null);
    fireEvent.change(screen.getByLabelText("Batch size"), { target: { value: "invalid" } });
    unmount();
    expect(useToastStore.getState().toasts.at(-1)?.severity).toBe("error");
  });
});
