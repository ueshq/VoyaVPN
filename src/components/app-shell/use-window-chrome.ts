import { useEffect, useState } from "react";

import { getWindowChromeConfig } from "@/ipc";
import type { WindowChromeConfig } from "@/ipc/bindings";

/** Web (no Tauri runtime) and the brief pre-resolve window both render native chrome. */
const DEFAULT_WINDOW_CHROME: WindowChromeConfig = {
  titleBarLayout: "none",
};

/**
 * Read the platform window chrome config from the backend. Falls back to `none`
 * (native frame, no custom title bar) on the web build or if the command fails.
 */
export function useWindowChrome(): WindowChromeConfig {
  const [chrome, setChrome] = useState<WindowChromeConfig>(DEFAULT_WINDOW_CHROME);

  useEffect(() => {
    let cancelled = false;

    getWindowChromeConfig()
      .then((config) => {
        if (!cancelled) setChrome(config);
      })
      .catch(() => {
        if (!cancelled) setChrome(DEFAULT_WINDOW_CHROME);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return chrome;
}
