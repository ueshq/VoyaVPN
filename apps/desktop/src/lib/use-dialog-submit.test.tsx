import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { AppError } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n";
import { IpcCommandError } from "@voya/client/errors";

import { useDialogSubmit } from "./use-dialog-submit";

vi.mock("@voya/client/messages", () => ({
  validationFieldErrors: (_t: TranslationFunction, issues: { field: string }[]) =>
    Object.fromEntries(issues.map((issue) => [issue.field, "invalid"])),
}));

const t = ((key: string) => key) as unknown as TranslationFunction;
// The real rejected-command shape: `appErrorOfKind` branches on the typed
// `kind`, so a plain object would prove nothing about what a backend sends.
const validationError = new IpcCommandError({
  kind: {
    issues: [{ code: { code: "textRequired" }, field: "name", scope: [] }],
    type: "validation",
  },
  message: "invalid",
  subsystem: "profile",
} satisfies AppError);

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
