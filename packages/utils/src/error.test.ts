import { describe, expect, it } from "vitest";

import { getErrorMessage } from "./error";

describe("getErrorMessage", () => {
  it("reads an error's message and stringifies anything else", () => {
    expect(getErrorMessage(new Error("core exited"))).toBe("core exited");
    expect(getErrorMessage("socket closed")).toBe("socket closed");
    expect(getErrorMessage(42)).toBe("42");
  });

  it("uses the fallback for an error without usable text", () => {
    expect(getErrorMessage(new Error("core exited"), "failed")).toBe("core exited");
    expect(getErrorMessage("socket closed", "failed")).toBe("socket closed");
    expect(getErrorMessage(new Error(""), "failed")).toBe("failed");
    expect(getErrorMessage("", "failed")).toBe("failed");
    expect(getErrorMessage({ code: 1 }, "failed")).toBe("failed");
  });
});
