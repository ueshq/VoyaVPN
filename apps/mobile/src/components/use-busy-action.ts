import { useCallback, useRef, useState } from "react";

/**
 * One action at a time, and a flag that says so.
 *
 * `busy` is what disables the controls, but a state write lands on the next
 * render: a second tap before that would start the action twice. The ref
 * refuses it at once. `run` resolves to what the action returned, or to
 * `undefined` when it was refused; a rejection is the action's own to report,
 * so it passes through.
 */
export function useBusyAction() {
  const [busy, setBusy] = useState(false);
  const pending = useRef(false);

  const run = useCallback(async <Result>(action: () => Promise<Result>) => {
    if (pending.current) return undefined;
    pending.current = true;
    setBusy(true);
    try {
      return await action();
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }, []);

  return { busy, run };
}
