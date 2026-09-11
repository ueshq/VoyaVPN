import { useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { assignNodeGroups, deleteNodeGroup, listNodeGroups, moveNodeGroup, saveNodeGroup } from "@/ipc";
import type { MoveAction, NodeGroup, NodeGroupAssignment, NodeGroupsSnapshot } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { getErrorMessage } from "@voya/utils/error";
import { UNASSIGNED_GROUP_KEY } from "./node-list-rows";

const EMPTY_GROUPS: NodeGroupsSnapshot = { groups: [], memberships: [] };
type GroupDialog =
  | { kind: "name"; group: NodeGroup | null }
  | { kind: "members" | "delete"; group: NodeGroup }
  | null;

export function useNodeGroups() {
  const query = useQuery({
    queryFn: listNodeGroups,
    queryKey: queryKeys.nodeGroups,
  });
  const snapshot = query.data ?? EMPTY_GROUPS;
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set([UNASSIGNED_GROUP_KEY]));
  const [dialog, setDialog] = useState<GroupDialog>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);
  const trigger = useRef<HTMLElement | null>(null);

  async function run<T>(operation: () => Promise<T>): Promise<{ value: T } | null> {
    if (pending.current) return null;
    pending.current = true;
    setBusy(true);
    setError(null);
    try {
      return { value: await operation() };
    } catch (cause) {
      setError(getErrorMessage(cause));
      return null;
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }

  function open(next: GroupDialog, element?: HTMLElement | null) {
    trigger.current = element ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
    setError(null);
    setDialog(next);
  }

  function toggle(id: string) {
    setExpanded((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  return {
    query,
    snapshot,
    expanded,
    dialog,
    error,
    busy,
    open,
    toggle,
    close() {
      if (!pending.current) setDialog(null);
    },
    restoreFocus(fallback: HTMLElement | null) {
      (trigger.current?.isConnected ? trigger.current : fallback)?.focus();
    },
    async saveName(id: string | null, name: string) {
      const result = await run(() => saveNodeGroup(id, name));
      if (result) {
        if (!id) setExpanded((previous) => new Set([...previous, result.value.id]));
        setDialog(null);
      }
    },
    async remove(id: string) {
      if (await run(() => deleteNodeGroup(id))) setDialog(null);
    },
    async move(id: string, action: MoveAction) {
      await run(() => moveNodeGroup(id, action));
    },
    async assign(assignments: NodeGroupAssignment[], close = false) {
      if (await run(() => assignNodeGroups(assignments))) {
        if (close) setDialog(null);
      }
    },
  };
}
