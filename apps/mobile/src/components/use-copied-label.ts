import { useCallback, useEffect, useRef, useState } from "react";

/**
 * The transient confirmation a copy button shows: `copied` flips true the
 * moment a write succeeds and back after `durationMs`, so the button can say
 * "Copied" instead of leaving the user to trust the clipboard silently.
 * `failed` is the same beat for a write that rejected — without it the button
 * says nothing and the tap reads as lost.
 *
 * No toast — the app shows none — and the reset timer is cleared on unmount,
 * so a screen that closes inside the window cannot setState on a dead tree.
 */
export function useCopiedLabel(durationMs = 2000) {
  const [copied, setCopied] = useState(false);
  const [failed, setFailed] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);

  const markCopied = useCallback(() => {
    setFailed(false);
    setCopied(true);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied(false), durationMs);
  }, [durationMs]);

  const markFailed = useCallback(() => {
    setCopied(false);
    setFailed(true);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setFailed(false), durationMs);
  }, [durationMs]);

  return { copied, failed, markCopied, markFailed };
}
