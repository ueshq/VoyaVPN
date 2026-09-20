import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { i18next } from "@voya/i18n";
import { deleteSubscriptions, updateSubscriptions } from "@/ipc/commands";
import type { Subscription, SubscriptionUpdateResult } from "@/ipc/bindings";
import { useNodeOperation } from "@voya/features/profiles/use-node-operation";
import { useNodeSubscriptions } from "./use-node-subscriptions";

vi.mock("@/ipc/commands", () => ({ deleteSubscriptions: vi.fn(), updateSubscriptions: vi.fn() }));

const source: Subscription = { id: "source", remarks: "Source", url: "https://example.test/sub", additionalUrl: "", userAgent: "", enabled: true, sort: 0, filter: null, converterTarget: null, autoUpdateIntervalMinutes: null };
const success: SubscriptionUpdateResult = { imported: 2, updated: 1, skipped: 0, removedExisting: 0, messages: [] };
function setup() {
  return renderHook(() => {
    const operation = useNodeOperation();
    return { ...operation, ...useNodeSubscriptions(operation, i18next.t.bind(i18next)) };
  });
}
beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

it("remembers an explicit or focused opener for the editor", () => {
  const { result } = setup();
  const trigger = document.createElement("button");
  document.body.append(trigger);
  trigger.focus();
  act(() => result.current.openSubscription(source));
  expect(result.current.subscriptionTriggerRef.current).toBe(trigger);
  expect(result.current.editingSubscription).toBe(source);
  expect(result.current.subscriptionsOpen).toBe(true);
  act(() => result.current.openSubscription(null, trigger));
  expect(result.current.editingSubscription).toBeNull();
  trigger.remove();
});

it("deduplicates an in-flight source update and releases it after completion", async () => {
  let finish!: (value: SubscriptionUpdateResult) => void;
  vi.mocked(updateSubscriptions).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const { result } = setup();
  let pending!: Promise<void>;
  act(() => { pending = result.current.updateSubscription(source.id); });
  expect(result.current.updatingSubscriptions.has(source.id)).toBe(true);
  await act(() => result.current.updateSubscription(source.id));
  expect(updateSubscriptions).toHaveBeenCalledTimes(1);
  await act(async () => { finish(success); await pending; });
  expect(result.current.updatingSubscriptions.size).toBe(0);
  expect(result.current.operationMessage).toBeTruthy();
});

it("reports a failed source update and permits a retry", async () => {
  vi.mocked(updateSubscriptions).mockResolvedValueOnce({ ...success, imported: 0, updated: 0, skipped: 1, messages: ["download failed"] }).mockResolvedValueOnce(success);
  const { result } = setup();
  await act(() => result.current.updateSubscription(source.id));
  expect(result.current.operationError).toBe("download failed");
  expect(result.current.updatingSubscriptions.size).toBe(0);
  await act(() => result.current.updateSubscription(source.id));
  expect(result.current.operationError).toBeNull();
});

it("explains a skipped update even when the backend supplied no message", async () => {
  vi.mocked(updateSubscriptions).mockResolvedValue({ ...success, imported: 0, updated: 0, skipped: 1 });
  const { result } = setup();
  await act(() => result.current.updateSubscription(source.id));
  expect(result.current.operationError).toBeTruthy();
});

it("keeps a failed deletion open and closes it only after a successful retry", async () => {
  vi.mocked(deleteSubscriptions).mockRejectedValueOnce(new Error("delete failed")).mockResolvedValueOnce(1);
  const { result } = setup();
  await act(() => result.current.removeSubscription());
  expect(deleteSubscriptions).not.toHaveBeenCalled();
  act(() => result.current.confirmSubscriptionDeletion(source, document.createElement("button")));
  await act(() => result.current.removeSubscription());
  expect(result.current.deletingSubscription).toBe(source);
  expect(result.current.deletingSubscriptionPending).toBe(false);
  expect(result.current.operationError).toBe("delete failed");
  await act(() => result.current.removeSubscription());
  expect(result.current.deletingSubscription).toBeNull();
});

it("guards deletion against a second click before the first one resolves", async () => {
  let finish!: (count: number) => void;
  vi.mocked(deleteSubscriptions).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const { result } = setup();
  act(() => result.current.confirmSubscriptionDeletion(source, document.createElement("button")));
  let pending!: Promise<void>;
  act(() => { pending = result.current.removeSubscription(); });
  expect(result.current.deletingSubscriptionPending).toBe(true);
  await act(() => result.current.removeSubscription());
  expect(deleteSubscriptions).toHaveBeenCalledTimes(1);
  await act(async () => { finish(1); await pending; });
  expect(result.current.deletingSubscriptionPending).toBe(false);
  expect(result.current.deletingSubscription).toBeNull();
});

it("updates every subscription at once and ignores a second request while running", async () => {
  let finish!: (value: SubscriptionUpdateResult) => void;
  vi.mocked(updateSubscriptions).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const { result } = setup();
  let pending!: Promise<void>;
  act(() => { pending = result.current.updateAllSubscriptions(); });
  expect(result.current.updatingAllSubscriptions).toBe(true);
  await act(() => result.current.updateAllSubscriptions());
  expect(updateSubscriptions).toHaveBeenCalledOnce();
  expect(updateSubscriptions).toHaveBeenCalledWith(null, true, null);
  await act(async () => { finish(success); await pending; });
  expect(result.current.updatingAllSubscriptions).toBe(false);
  expect(result.current.operationMessage).toBeTruthy();
});
