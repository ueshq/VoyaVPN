import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { i18next } from "@voya/i18n";
import type { Subscription, SubscriptionUpdateResult } from "@voya/contracts";
import { useNodeOperation } from "./use-node-operation";

import { installFakeCommands } from "../test/backend";
import { useNodeSubscriptions } from "./use-node-subscriptions";

// The hook reaches the backend through the shared seam, so the test registers
// a surface rather than mocking a module.
const ipc = installFakeCommands({
  deleteSubscriptions: vi.fn(),
  updateSubscriptions: vi.fn(),
});

const source: Subscription = { id: "source", remarks: "Source", url: "https://example.test/sub", additionalUrl: "", userAgent: "", enabled: true, sort: 0, filter: null, converterTarget: null, autoUpdateIntervalMinutes: null };
const success: SubscriptionUpdateResult = { imported: 2, updated: 1, skipped: 0, removedExisting: 0, messages: [] };
function setup() {
  return renderHook(() => {
    const operation = useNodeOperation();
    return {
      ...operation,
      ...useNodeSubscriptions<string>(operation, i18next.t.bind(i18next)),
    };
  });
}
beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

it("carries whatever the caller wants focus returned to, and forgets it otherwise", () => {
  const { result } = setup();

  // The opener is the caller's to name: an element on the desktop, nothing on
  // a phone, where a sheet dismisses back to the list on its own.
  act(() => result.current.openSubscription(source, "add-button"));
  expect(result.current.subscriptionTriggerRef.current).toBe("add-button");
  expect(result.current.editingSubscription).toBe(source);
  expect(result.current.subscriptionsOpen).toBe(true);

  act(() => result.current.openSubscription(null));
  expect(result.current.subscriptionTriggerRef.current).toBeNull();
  expect(result.current.editingSubscription).toBeNull();
});

it("deduplicates an in-flight source update and releases it after completion", async () => {
  let finish!: (value: SubscriptionUpdateResult) => void;
  ipc.updateSubscriptions.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const { result } = setup();
  let pending!: Promise<void>;
  act(() => { pending = result.current.updateSubscription(source.id); });
  expect(result.current.updatingSubscriptions.has(source.id)).toBe(true);
  await act(() => result.current.updateSubscription(source.id));
  expect(ipc.updateSubscriptions).toHaveBeenCalledTimes(1);
  await act(async () => { finish(success); await pending; });
  expect(result.current.updatingSubscriptions.size).toBe(0);
  expect(result.current.operationMessage).toBeTruthy();
});

it("reports a failed source update and permits a retry", async () => {
  ipc.updateSubscriptions.mockResolvedValueOnce({ ...success, imported: 0, updated: 0, skipped: 1, messages: ["download failed"] }).mockResolvedValueOnce(success);
  const { result } = setup();
  await act(() => result.current.updateSubscription(source.id));
  expect(result.current.operationError).toBe("download failed");
  expect(result.current.updatingSubscriptions.size).toBe(0);
  await act(() => result.current.updateSubscription(source.id));
  expect(result.current.operationError).toBeNull();
});

it("explains a skipped update even when the backend supplied no message", async () => {
  ipc.updateSubscriptions.mockResolvedValue({ ...success, imported: 0, updated: 0, skipped: 1 });
  const { result } = setup();
  await act(() => result.current.updateSubscription(source.id));
  expect(result.current.operationError).toBeTruthy();
});

it("keeps a failed deletion open and closes it only after a successful retry", async () => {
  ipc.deleteSubscriptions.mockRejectedValueOnce(new Error("delete failed")).mockResolvedValueOnce(1);
  const { result } = setup();
  await act(() => result.current.removeSubscription());
  expect(ipc.deleteSubscriptions).not.toHaveBeenCalled();
  act(() => result.current.confirmSubscriptionDeletion(source, "delete-button"));
  await act(() => result.current.removeSubscription());
  expect(result.current.deletingSubscription).toBe(source);
  expect(result.current.deletingSubscriptionPending).toBe(false);
  expect(result.current.operationError).toBe("delete failed");
  await act(() => result.current.removeSubscription());
  expect(result.current.deletingSubscription).toBeNull();
});

it("guards deletion against a second click before the first one resolves", async () => {
  let finish!: (count: number) => void;
  ipc.deleteSubscriptions.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const { result } = setup();
  act(() => result.current.confirmSubscriptionDeletion(source, "delete-button"));
  let pending!: Promise<void>;
  act(() => { pending = result.current.removeSubscription(); });
  expect(result.current.deletingSubscriptionPending).toBe(true);
  await act(() => result.current.removeSubscription());
  expect(ipc.deleteSubscriptions).toHaveBeenCalledTimes(1);
  await act(async () => { finish(1); await pending; });
  expect(result.current.deletingSubscriptionPending).toBe(false);
  expect(result.current.deletingSubscription).toBeNull();
});

it("updates every subscription at once and ignores a second request while running", async () => {
  let finish!: (value: SubscriptionUpdateResult) => void;
  ipc.updateSubscriptions.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const { result } = setup();
  let pending!: Promise<void>;
  act(() => { pending = result.current.updateAllSubscriptions(); });
  expect(result.current.updatingAllSubscriptions).toBe(true);
  await act(() => result.current.updateAllSubscriptions());
  expect(ipc.updateSubscriptions).toHaveBeenCalledOnce();
  expect(ipc.updateSubscriptions).toHaveBeenCalledWith(null, true, null);
  await act(async () => { finish(success); await pending; });
  expect(result.current.updatingAllSubscriptions).toBe(false);
  expect(result.current.operationMessage).toBeTruthy();
});
