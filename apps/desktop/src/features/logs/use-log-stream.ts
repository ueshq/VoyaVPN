import { useEffect } from "react";

import { setLogStreaming } from "@/ipc/commands";
import { isTauriRuntime } from "@/ipc/window";
import { useAppVisible } from "@voya/client/use-app-visible";

/**
 * Has the backend deliver log lines while the calling panel is mounted and the
 * window is on screen. Otherwise it keeps the newest few hundred instead of
 * serializing every batch into a webview that shows none of them, and hands
 * those over when streaming resumes, so opening the panel still shows what
 * just happened.
 */
export function useLogStream() {
  const visible = useAppVisible();

  useEffect(() => {
    if (!visible || !isTauriRuntime()) {
      return undefined;
    }

    // Cannot fail in the backend; a lost call only delays or repeats lines.
    void setLogStreaming(true).catch(() => undefined);
    return () => {
      void setLogStreaming(false).catch(() => undefined);
    };
  }, [visible]);
}
