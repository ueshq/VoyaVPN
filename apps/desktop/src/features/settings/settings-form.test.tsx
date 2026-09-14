import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Input } from "@voya/ui/components/input";
import { NumberField, SettingsFields, SettingsGroup, SettingsRow, SettingsSwitch } from "./settings-form";
import { useToastStore } from "@/stores/toast-store";

describe("settings form", () => {
  it("associates row labels and groups with accessible controls", () => {
    render(<SettingsGroup title="Startup"><SettingsRow htmlFor="field" label="Name"><Input id="field" /></SettingsRow>
      <SettingsFields errors={{ autostart: "Not allowed" }}><SettingsSwitch field="autostart" checked label="Autostart" onCheckedChange={vi.fn()} /></SettingsFields>
    </SettingsGroup>);
    expect(screen.getByRole("region", { name: "Startup" })).toBeInTheDocument();
    expect(screen.getByLabelText("Name")).toHaveAttribute("id", "field");
    expect(screen.getByRole("switch", { name: "Autostart" })).toHaveAccessibleDescription("Not allowed");
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

  it("clears nullable numbers and keeps a typing mistake out of the toasts", () => {
    useToastStore.setState({ toasts: [] });
    const onChange = vi.fn();
    const { unmount } = render(<NumberField field="pageSize" label="Batch size" nullable value={10} onChange={onChange} />);
    fireEvent.change(screen.getByLabelText("Batch size"), { target: { value: "" } });
    fireEvent.blur(screen.getByLabelText("Batch size"));
    expect(onChange).toHaveBeenCalledWith(null);
    fireEvent.change(screen.getByLabelText("Batch size"), { target: { value: "invalid" } });
    unmount();
    expect(useToastStore.getState().toasts).toHaveLength(0);
  });

  it("restores the default when a number that has one is cleared", () => {
    const onChange = vi.fn();
    render(<NumberField defaultValue={1500} field="mtu" label="MTU" value={9000} onChange={onChange} />);
    const input = screen.getByLabelText("MTU");
    expect(input).toHaveAccessibleDescription("Default: 1500");
    fireEvent.change(input, { target: { value: "" } });
    fireEvent.blur(input);
    expect(onChange).toHaveBeenCalledWith(1500);
    expect(input).not.toHaveAttribute("aria-invalid");
  });
});
