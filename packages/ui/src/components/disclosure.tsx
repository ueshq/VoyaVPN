import { useEffect, useRef, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "../lib/utils";

/** Keeps drafts mounted while progressively disclosing advanced fields. */
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
    <details
      ref={ref}
      className={cn(
        "group/disclosure min-w-0 rounded-xl border bg-surface-raised",
        className,
      )}
    >
      <summary className="flex cursor-pointer list-none items-center gap-2 rounded-xl p-4 text-sm font-medium outline-none focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden">
        <ChevronRight
          aria-hidden="true"
          className="size-4 transition-transform duration-short group-open/disclosure:rotate-90"
        />
        {title}
      </summary>
      <div className="grid gap-4 border-t p-4">{children}</div>
    </details>
  );
}
