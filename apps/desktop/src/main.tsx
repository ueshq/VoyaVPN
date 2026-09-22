// Side-effect first: registers storage, the system colour scheme and the Tauri
// command binding behind @voya/client, before any store module is evaluated.
import "./platform-boot";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";

import { localeReady } from "@voya/i18n";

import { App } from "./App";
import { AppErrorBoundary } from "@/components/app-shell/error-boundary";
import { createAppQueryClient } from "@voya/client/query-client";
import "./styles/globals.css";

const queryClient = createAppQueryClient({ refetchOnWindowFocus: false });

function render() {
  createRoot(document.getElementById("root")!, {
    // A render error that escapes every boundary blanks the window; log it so
    // the webview console still carries a diagnostic.
    onUncaughtError: (error, errorInfo) => {
      console.error("[app] uncaught render error", error, errorInfo.componentStack);
    },
  }).render(
    <StrictMode>
      <AppErrorBoundary>
        <QueryClientProvider client={queryClient}>
          <App />
        </QueryClientProvider>
      </AppErrorBoundary>
    </StrictMode>,
  );
}

// Only a non-English startup waits, for its own locale chunk.
void localeReady
  .catch((error: unknown) => {
    console.error("[app] failed to load the startup locale", error);
  })
  .then(render);
