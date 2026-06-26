import type * as React from "react";
import { TriangleAlert } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { cn } from "@/lib/utils";

// The destructive banner that four screens (Profiles, Routing, Clash proxies,
// Clash connections) each hand-rolled below their page header to surface an
// operation/query error. It spans the content edge-to-edge — no outer rounding,
// only a bottom rule — so it reads as a band stacked under the header rather
// than a floating card. `role="alert"` and the destructive styling come from the
// underlying Alert primitive. Pass `icon={null}` to omit the leading glyph.
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
      className={cn("rounded-none border-x-0 border-t-0 px-4 py-2", className)}
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
