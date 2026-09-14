import * as React from "react";
import { useShellStore } from "@/stores/shell-store";

import { cn } from "@voya/ui/lib/utils";

// Page identity and content share one inset. Only the feature's inner viewport
// scrolls: the page frame must not introduce another scroll container.
export const pageSurfaceClassName = "rounded-xl border-0 bg-surface-raised";

function PageSection({ className, ...props }: React.ComponentProps<"section">) {
  return (
    <section
      className={cn("flex h-full min-h-0 min-w-0 flex-col bg-surface-canvas", className)}
      data-slot="page-section"
      {...props}
    />
  );
}

// One h1 and its page-level actions; panel-specific tools use PageHeader below.
function PageTitle({
  actions,
  className,
  title,
  ...props
}: React.ComponentProps<"div"> & {
  actions?: React.ReactNode;
  title: React.ReactNode;
}) {
  const titleRef = React.useRef<HTMLHeadingElement>(null);
  const focusTitle = useShellStore((state) => state.focusPageTitle);
  React.useEffect(() => {
    if (focusTitle) {
      titleRef.current?.focus();
      useShellStore.getState().consumeFocusPageTitle();
    }
  }, [focusTitle]);
  return (
    <div
      className={cn(
        "flex min-w-0 shrink-0 items-center gap-3 px-4 pt-5 pb-4 min-[1100px]:px-page",
        className,
      )}
      data-slot="page-title"
      {...props}
    >
      <h1
        ref={titleRef}
        tabIndex={-1}
        className="min-w-0 shrink-0 text-page font-semibold tracking-tight outline-none"
      >
        {title}
      </h1>
      {actions ? (
        <PageHeaderActions className="min-h-9">{actions}</PageHeaderActions>
      ) : null}
    </div>
  );
}

function PageContent({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex min-h-0 min-w-0 flex-1 flex-col gap-4 px-4 pb-4 min-[1100px]:px-page min-[1100px]:pb-page",
        className,
      )}
      data-slot="page-content"
      {...props}
    />
  );
}

function PageSurface({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(pageSurfaceClassName, "min-h-0 min-w-0", className)}
      data-slot="page-surface"
      {...props}
    />
  );
}

// A toolbar inside a content panel: padding stays local to the panel rather
// than growing with the page inset at wider window sizes.
function PageHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "flex min-h-12 shrink-0 flex-wrap items-center gap-2 bg-surface-raised px-4 py-2",
        className,
      )}
      data-slot="page-header"
      {...props}
    />
  );
}

// Trailing toolbar cluster: parks controls against the header's end edge with the
// canonical `ms-auto` push so screens stop re-deriving it inline.
function PageHeaderActions({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      className={cn("ms-auto flex min-w-0 flex-wrap items-center justify-end gap-2", className)}
      data-slot="page-header-actions"
      {...props}
    />
  );
}

export { PageContent, PageHeader, PageHeaderActions, PageSection, PageSurface, PageTitle };
