import { useState } from "react";

export function useNodeGroups() {
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  function toggle(id: string) {
    setCollapsed((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }
  return { collapsed, toggle };
}
