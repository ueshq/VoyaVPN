import { useEffect, useLayoutEffect, useRef, useState } from "react";

/** Optional commit-on-blur input behavior; ordinary shared fields stay controlled. */
export function useCommittedInput({ value, onChange, deferred, validate, onInvalid }: {
  value: string;
  onChange: (value: string) => void;
  deferred: boolean;
  validate?: (value: string) => string | undefined;
  onInvalid?: (message: string, leaving: boolean) => void;
}) {
  const [local, setLocal] = useState<string | null>(null);
  const [error, setError] = useState<string>();
  const pending = useRef<string | null>(null);
  const composing = useRef(false);
  const blurredWhileComposing = useRef(false);
  const latest = useRef({ onChange, validate, onInvalid });
  useLayoutEffect(() => { latest.current = { onChange, validate, onInvalid }; });

  function commit(leaving = false) {
    const text = pending.current;
    if (text === null || composing.current) return;
    const message = latest.current.validate?.(text);
    if (message) {
      if (!leaving) setError(message);
      latest.current.onInvalid?.(message, leaving);
      return;
    }
    pending.current = null;
    latest.current.onChange(text);
    if (!leaving) { setLocal(null); setError(undefined); }
  }

  const flush = useRef(commit);
  useLayoutEffect(() => { flush.current = commit; });
  useEffect(() => () => { flush.current(true); }, []);

  return {
    value: local ?? value,
    error,
    onChange: (text: string) => {
      if (!deferred) { onChange(text); return; }
      pending.current = text;
      setLocal(text);
      setError(undefined);
    },
    onBlur: () => {
      blurredWhileComposing.current = composing.current;
      commit();
    },
    onKeyDown: (event: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
      if (deferred && event.key === "Enter" && !event.nativeEvent.isComposing && !composing.current && event.currentTarget.tagName !== "TEXTAREA") {
        event.preventDefault();
        commit();
      }
    },
    onCompositionStart: () => { composing.current = true; },
    onCompositionEnd: () => {
      composing.current = false;
      if (blurredWhileComposing.current) { blurredWhileComposing.current = false; commit(); }
    },
  };
}
