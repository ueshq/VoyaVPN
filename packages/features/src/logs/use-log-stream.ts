import { useEffect } from "react";

import { voyaCommands } from "@voya/client/transport";
import { useAppVisible } from "@voya/client/use-app-visible";

/**
 * Has the backend deliver log lines while the calling screen is mounted and the
 * app is on screen. Otherwise it keeps the newest few hundred instead of
 * serializing every batch into a webview that shows none of them, and hands
 * those over when streaming resumes, so opening the panel still shows what
 * just happened.
 */
export function useLogStream(enabled = true) {
  const visible = useAppVisible();

  useEffect(() => {
    if (!enabled || !visible) {
      return undefined;
    }

    // Cannot fail in the backend; a lost call only delays or repeats lines.
    void voyaCommands().setLogStreaming(true).catch(() => undefined);
    return () => {
      void voyaCommands().setLogStreaming(false).catch(() => undefined);
    };
  }, [visible, enabled]);
}
