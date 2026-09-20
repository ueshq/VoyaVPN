import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { TranslationFunction } from "@voya/i18n";

import { useDialogSubmit } from "./use-dialog-submit";

vi.mock("@/ipc/commands", () => ({
  appErrorOfKind: (error: unknown, type: string) =>
    (error as { kind?: { type?: string } }).kind?.type === type ? error : null,
}));
vi.mock("@voya/client/messages", () => ({
  validationFieldErrors: (_t: TranslationFunction, issues: { field: string }[]) =>
    Object.fromEntries(issues.map((issue) => [issue.field, "invalid"])),
}));

const t = ((key: string) => key) as unknown as TranslationFunction;
const validationError = { kind: { issues: [{ field: "name" }], type: "validation" }, message: "invalid" };

describe("useDialogSubmit", () => {
  it("keeps a failed save's message and clears it on the next attempt", async () => {
    const { result } = renderHook(() => useDialogSubmit());

    await act(() => result.current.submit(() => Promise.reject(new Error("offline"))));
    expect(result.current.error).toContain("offline");
    expect(result.current.pending).toBe(false);

    const save = vi.fn(() => Promise.resolve());
    await act(() => result.current.submit(save));
    expect(save).toHaveBeenCalledOnce();
    expect(result.current).toMatchObject({ error: null, fieldErrors: {}, pending: false });
  });

  it("maps validation issues onto fields only for a dialog that shows them", async () => {
    const withFields = renderHook(() => useDialogSubmit(t));
    await act(() => withFields.result.current.submit(() => Promise.reject(validationError)));
    expect(withFields.result.current).toMatchObject({ error: null, fieldErrors: { name: "invalid" } });

    const withoutFields = renderHook(() => useDialogSubmit());
    await act(() => withoutFields.result.current.submit(() => Promise.reject(validationError)));
    expect(withoutFields.result.current.fieldErrors).toEqual({});
    expect(withoutFields.result.current.error).toEqual(expect.any(String));
  });
});
