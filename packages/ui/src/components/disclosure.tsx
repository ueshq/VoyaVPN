import { useEffect, useRef, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "../lib/utils";

/**
 * Keeps drafts mounted while progressively disclosing advanced fields. It has
 * no border or fill of its own, so it reads as part of the panel or dialog it
 * sits in instead of a card inside a card.
 */
export function Disclosure({
  title,
  children,
  invalid = false,
  className,
}: {
  title: string;
  children: ReactNode;
  invalid?: boolean;
  className?: string;
}) {
  const ref = useRef<HTMLDetailsElement>(null);
  useEffect(() => {
    if (invalid && ref.current) ref.current.open = true;
  }, [invalid]);
  return (
    <details ref={ref} className={cn("group/disclosure min-w-0", className)}>
      <summary className="flex w-fit cursor-pointer list-none items-center gap-2 rounded-md py-1 text-sm font-medium text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden">
        <ChevronRight
          aria-hidden="true"
          className="size-4 text-muted-foreground transition-transform duration-short group-open/disclosure:rotate-90"
        />
        {title}
      </summary>
      <div className="grid gap-4 pt-3">{children}</div>
    </details>
  );
}
