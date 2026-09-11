import { Component, type ErrorInfo, type ReactNode } from "react";
import { TriangleAlert } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { useI18n } from "@voya/i18n/use-i18n";

type AppErrorBoundaryProps = {
  children: ReactNode;
  /**
   * Changing this value clears a caught error. The shell passes the active tab
   * so navigating away from a crashed screen recovers without a reload.
   */
  resetKey?: string;
};

type AppErrorBoundaryState = {
  error: Error | null;
  resetKey: string | undefined;
};

/**
 * React unmounts the whole root when a render error reaches it, which for a
 * tray-resident client means a blank window with no way back. This boundary
 * keeps a throwing screen (or a rejected lazy chunk) contained: the shell
 * around it survives and the user gets a localized explanation plus a retry.
 *
 * It must be a class component, so the fallback lives in a function component
 * that can call {@link useI18n} — no user-visible string is hardcoded here.
 */
export class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  constructor(props: AppErrorBoundaryProps) {
    super(props);
    this.state = { error: null, resetKey: props.resetKey };
    this.retry = this.retry.bind(this);
  }

  static getDerivedStateFromError(error: Error): Partial<AppErrorBoundaryState> {
    return { error };
  }

  static getDerivedStateFromProps(
    props: AppErrorBoundaryProps,
    state: AppErrorBoundaryState,
  ): Partial<AppErrorBoundaryState> | null {
    if (props.resetKey !== state.resetKey) {
      return { error: null, resetKey: props.resetKey };
    }

    return null;
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[app-shell] render error", error, info.componentStack);
  }

  retry() {
    this.setState({ error: null });
  }

  render() {
    if (this.state.error) {
      return <AppErrorFallback onRetry={this.retry} />;
    }

    return this.props.children;
  }
}

function AppErrorFallback({ onRetry }: { onRetry: () => void }) {
  const { t } = useI18n();

  return (
    <div
      className="flex h-full min-h-48 flex-col items-center justify-center gap-3 p-6 text-center"
      data-testid="app-error-fallback"
      role="alert"
    >
      <TriangleAlert aria-hidden="true" className="size-8 text-danger" />
      <p className="font-display text-base font-semibold">{t("status.screenErrorTitle")}</p>
      <p className="max-w-md text-sm text-muted-foreground">{t("status.screenErrorDescription")}</p>
      <div className="flex items-center gap-2">
        <Button onClick={onRetry} size="sm" type="button">
          {t("actions.retry")}
        </Button>
        <Button
          onClick={() => window.location.reload()}
          size="sm"
          type="button"
          variant="outline"
        >
          {t("actions.reload")}
        </Button>
      </div>
    </div>
  );
}
