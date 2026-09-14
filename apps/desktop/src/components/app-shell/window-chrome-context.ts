import { createContext } from "react";

import type { TitleBarLayout } from "@/ipc/bindings";

/** The shell's window chrome, for page parts that double as the macOS titlebar. */
export const WindowChromeContext = createContext<TitleBarLayout>("none");
