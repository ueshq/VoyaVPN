import type * as React from "react";
import { LoaderCircle } from "lucide-react";

import { cn } from "@voya/ui/lib/utils";

/** A decorative spinning loader; the text next to it says what is happening. */
function Spinner({ className, ...props }: React.ComponentProps<typeof LoaderCircle>) {
  return <LoaderCircle aria-hidden="true" className={cn(className, "animate-spin")} {...props} />;
}

export { Spinner };
