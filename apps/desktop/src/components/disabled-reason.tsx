import type { ReactNode } from "react";

/**
 * Says why a control is disabled. A disabled button shows no tooltip of its own,
 * so the reason sits on a wrapper the pointer can still reach. The wrapper is
 * always rendered, so a reason coming and going never remounts the control.
 */
export function DisabledReason({
  children,
  reason,
}: {
  children: ReactNode;
  reason?: string | null;
}) {
  return (
    <span className="inline-flex" title={reason ?? undefined}>
      {children}
    </span>
  );
}
