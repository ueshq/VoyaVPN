import { beforeEach, describe, expect, it } from "vitest";

import { toastError, useToastStore } from "./toast-store";

describe("toast store", () => {
  beforeEach(() => {
    useToastStore.setState({ toasts: [] });
  });

  it("reports an error with the URL in its text redacted", () => {
    toastError("Update failed", new Error("fetch https://user:secret@example.test/sub failed"));

    expect(useToastStore.getState().toasts).toEqual([
      expect.objectContaining({
        description: "fetch [redacted URL] failed",
        severity: "error",
        title: "Update failed",
      }),
    ]);
  });
});
