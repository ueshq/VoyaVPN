import { beforeEach, describe, expect, it } from "vitest";

import { useToastStore } from "@/stores/toast-store";

import { createAppQueryClient } from "./query-client";

describe("createAppQueryClient", () => {
  beforeEach(() => {
    useToastStore.setState({ toasts: [] });
  });

  it("stops retrying deterministic IPC failures and focus refetching", () => {
    const queryClient = createAppQueryClient();

    expect(queryClient.getDefaultOptions().queries).toMatchObject({
      refetchOnWindowFocus: false,
      retry: false,
      staleTime: 30_000,
    });
  });

  it("surfaces a rejected mutation as a toast titled by the feature", async () => {
    const queryClient = createAppQueryClient();
    const mutation = queryClient.getMutationCache().build<void, Error, void, unknown>(queryClient, {
      meta: { errorTitle: "Failed to reload core configuration" },
      mutationFn: () => Promise.reject(new Error("clash api unreachable")),
    });

    await expect(mutation.execute(undefined)).rejects.toThrow("clash api unreachable");

    expect(useToastStore.getState().toasts.at(-1)).toMatchObject({
      description: "clash api unreachable",
      severity: "error",
      title: "Failed to reload core configuration",
    });
  });

  it("falls back to the generic operation title and redacts the message", async () => {
    const queryClient = createAppQueryClient();
    const mutation = queryClient.getMutationCache().build<void, Error, void, unknown>(queryClient, {
      mutationFn: () =>
        Promise.reject(new Error("request failed https://user:secret@example.test/sub")),
    });

    await expect(mutation.execute(undefined)).rejects.toThrow("request failed");

    const toast = useToastStore.getState().toasts.at(-1);
    expect(toast).toMatchObject({ severity: "error", title: "Operation failed" });
    expect(toast?.description).not.toContain("secret");
  });
});
