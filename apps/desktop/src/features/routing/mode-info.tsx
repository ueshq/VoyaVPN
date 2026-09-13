import { Info } from "lucide-react";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@voya/ui/components/tooltip";

export function ModeInfo({ label, hint }: { label: string; hint: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          aria-label={label}
          className="inline-flex size-5 shrink-0 items-center justify-center rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
          type="button"
        >
          <Info aria-hidden="true" className="size-3.5" />
        </button>
      </TooltipTrigger>
      {/* Hints put each mode on its own line. */}
      <TooltipContent className="whitespace-pre-line">{hint}</TooltipContent>
    </Tooltip>
  );
}
