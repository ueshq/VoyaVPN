import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";

import { App } from "./App";
import { AppErrorBoundary } from "@/components/app-shell/error-boundary";
import { createAppQueryClient } from "@/components/app-shell/query-client";
import "./styles/globals.css";

const queryClient = createAppQueryClient();

createRoot(document.getElementById("root")!, {
  // A render error that escapes every boundary blanks the window; log it so the
  // webview console still carries a diagnostic.
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
