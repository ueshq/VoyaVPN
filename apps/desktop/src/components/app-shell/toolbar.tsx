import type * as React from "react";

import { cn } from "@voya/ui/lib/utils";

// Accessible page-level action row, composed inside PageTitle.

function Toolbar({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn("flex flex-wrap items-center gap-2", className)}
      data-slot="toolbar"
      role="toolbar"
      {...props}
    />
  );
}

export { Toolbar };
