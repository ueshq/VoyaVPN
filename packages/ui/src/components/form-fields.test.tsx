import { useState } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { CheckboxField, SelectField, TextAreaField, TextField } from "./form-fields";

afterEach(cleanup);

describe("form fields", () => {
  it("associates translated labels and errors with distinct inputs", () => {
    const onChange = vi.fn();
    render(<>
      <TextField label="地址" value="" onChange={onChange} error="地址不能为空" />
      <TextAreaField label="域名" value="" onChange={onChange} error="域名无效" />
    </>);
    const address = screen.getByLabelText("地址");
    const domains = screen.getByLabelText("域名");
    expect(address.id).not.toBe(domains.id);
    expect(address).toHaveAccessibleDescription("地址不能为空");
    expect(domains).toHaveAccessibleDescription("域名无效");
    expect(address).toHaveAttribute("aria-invalid", "true");
    fireEvent.change(domains, { target: { value: "example.com\nexample.org" } });
    expect(onChange).toHaveBeenCalledWith("example.com\nexample.org");
  });

  it("round-trips the empty select value without exposing the sentinel", async () => {
    const onChange = vi.fn();
    function Form() {
      const [value, setValue] = useState("");
      return <SelectField label="策略" value={value} onChange={(next) => { onChange(next); setValue(next); }} options={[
        { label: "默认", value: "" },
        { label: "IPv4", value: "preferIpv4" },
      ]} />;
    }
    render(<Form />);
    const select = screen.getByRole("combobox", { name: "策略" });
    expect(select).toHaveTextContent("默认");
    fireEvent.keyDown(select, { key: "Enter" });
    fireEvent.keyDown(await screen.findByRole("option", { name: "IPv4" }), { key: "Enter" });
    expect(onChange).toHaveBeenLastCalledWith("preferIpv4");
    fireEvent.keyDown(select, { key: "Enter" });
    fireEvent.keyDown(await screen.findByRole("option", { name: "默认" }), { key: "Enter" });
    expect(onChange).toHaveBeenLastCalledWith("");
  });

  it("honors explicit ids and disabled controls", () => {
    const onChange = vi.fn();
    render(<>
      <TextField id="address" label="地址" value="saved" onChange={onChange} disabled />
      <SelectField label="策略" value="" onChange={onChange} options={[{ label: "默认", value: "" }]} disabled />
      <CheckboxField label="全局映射" checked={false} onChange={onChange} disabled />
      <CheckboxField label="启用" checked={false} onChange={onChange} />
    </>);
    expect(screen.getByLabelText("地址")).toHaveAttribute("id", "address");
    expect(screen.getByLabelText("地址")).toBeDisabled();
    expect(screen.getByRole("combobox")).toBeDisabled();
    fireEvent.click(screen.getByLabelText("全局映射"));
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.click(screen.getByLabelText("启用"));
    expect(onChange).toHaveBeenCalledWith(true);
  });
});

describe("commit-on-blur fields", () => {
  it("defers input, ignores IME Enter, commits once after composition, and preserves ordinary fields", () => {
    const onChange = vi.fn();
    render(<TextField commitOnBlur label="Name" value="" onChange={onChange} />);
    const input = screen.getByLabelText("Name");
    fireEvent.compositionStart(input);
    fireEvent.change(input, { target: { value: "中文" } });
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.compositionEnd(input);
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.blur(input);
    expect(onChange).toHaveBeenCalledExactlyOnceWith("中文");
  });

  it("commits multiline text only on blur and flushes focused input on unmount", () => {
    const onChange = vi.fn();
    const { unmount } = render(<TextAreaField commitOnBlur label="Hosts" value="" onChange={onChange} />);
    const input = screen.getByLabelText("Hosts");
    fireEvent.change(input, { target: { value: "example.com 127.0.0.1" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).not.toHaveBeenCalled();
    unmount();
    expect(onChange).toHaveBeenCalledExactlyOnceWith("example.com 127.0.0.1");
  });
});

it("waits for composition to finish after blur and never submits an unfinished composition on unmount", () => {
  const onChange = vi.fn();
  const { unmount } = render(<TextField commitOnBlur label="IME" value="" onChange={onChange} />);
  const input = screen.getByLabelText("IME");
  fireEvent.compositionStart(input);
  fireEvent.change(input, { target: { value: "中文" } });
  fireEvent.blur(input);
  expect(onChange).not.toHaveBeenCalled();
  fireEvent.compositionEnd(input);
  expect(onChange).toHaveBeenCalledExactlyOnceWith("中文");
  fireEvent.compositionStart(input);
  fireEvent.change(input, { target: { value: "未完成" } });
  unmount();
  expect(onChange).toHaveBeenCalledTimes(1);
});
