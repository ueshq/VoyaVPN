import { useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  assignNodeGroups,
  deleteNodeGroup,
  listNodeGroups,
  moveNodeGroup,
  saveNodeGroup,
  updateNodeGroup,
} from "@/ipc/commands";
import type { MoveAction, NodeGroup, NodeGroupAssignment, NodeGroupsSnapshot } from "@/ipc/bindings";
import { queryKeys } from "@/ipc/query-keys";
import { getErrorMessage } from "@voya/utils/error";

const EMPTY_GROUPS: NodeGroupsSnapshot = { groups: [], memberships: [] };
type GroupDialog =
  | { kind: "name"; group: NodeGroup | null }
  | { kind: "edit" | "delete"; group: NodeGroup }
  | null;

export function useNodeGroups() {
  const query = useQuery({
    queryFn: listNodeGroups,
    queryKey: queryKeys.nodeGroups,
  });
  const snapshot = query.data ?? EMPTY_GROUPS;
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
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
    setCollapsed((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  return {
    query,
    snapshot,
    collapsed,
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
        setDialog(null);
      }
    },
    async update(id: string, name: string, assignments: NodeGroupAssignment[]) {
      if (await run(() => updateNodeGroup(id, name, assignments))) setDialog(null);
    },
    async remove(id: string) {
      if (await run(() => deleteNodeGroup(id))) setDialog(null);
    },
    async move(id: string, action: MoveAction) {
      await run(() => moveNodeGroup(id, action));
    },
    async assign(assignments: NodeGroupAssignment[]) {
      await run(() => assignNodeGroups(assignments));
    },
  };
}
