import type * as React from "react";
import { TriangleAlert } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { cn } from "@voya/ui/lib/utils";
import { pageSurfaceClassName } from "./page-section";

// Errors occupy the same inset and rounded surface as other page content.
// Alert supplies the accessible role and destructive foreground.
function InlinePageError({
  children,
  className,
  icon: Icon = TriangleAlert,
  ...props
}: React.ComponentProps<"div"> & {
  icon?: LucideIcon | null;
}) {
  return (
    <Alert
      className={cn(pageSurfaceClassName, "shrink-0 px-4 py-2", className)}
      data-slot="inline-page-error"
      variant="destructive"
      {...props}
    >
      {Icon ? <Icon aria-hidden="true" /> : null}
      <AlertDescription>{children}</AlertDescription>
    </Alert>
  );
}

export { InlinePageError };
