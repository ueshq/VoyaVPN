import type * as React from "react";
import { Search, X } from "lucide-react";

import { Button } from "@voya/ui/components/button";
import { Input } from "@voya/ui/components/input";
import { cn } from "@voya/ui/lib/utils";

type SearchInputProps = Omit<React.ComponentProps<"input">, "type"> & {
  /** Announced and used as the placeholder: one phrase per caller. */
  label: string;
  /** Announced on the clear button shown while the field has a value. */
  clearLabel: string;
  /** Clear the controlled value; an Escape press clears too. */
  onClear: () => void;
};

/** A filter field: search icon, an explicit clear button, Escape to clear. */
function SearchInput({
  className,
  clearLabel,
  label,
  onChange,
  onClear,
  onKeyDown,
  ref,
  value,
  ...props
}: SearchInputProps) {
  const hasValue = typeof value === "string" && value.length > 0;
  return (
    <div className={cn("relative min-w-0 flex-1", className)}>
      <Search
        aria-hidden="true"
        className="pointer-events-none absolute start-3 top-2.5 size-4 text-muted-foreground"
      />
      <Input
        {...props}
        aria-label={label}
        className="ps-9 pe-10 [&::-webkit-search-cancel-button]:hidden"
        onChange={onChange}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.stopPropagation();
            onClear();
          } else {
            onKeyDown?.(event);
          }
        }}
        placeholder={label}
        ref={ref}
        type="search"
        value={value}
      />
      {hasValue ? (
        <Button
          aria-label={clearLabel}
          className="absolute end-0.5 top-0.5 size-8"
          onClick={onClear}
          size="icon-sm"
          type="button"
          variant="ghost"
        >
          <X aria-hidden="true" className="size-4" />
        </Button>
      ) : null}
    </div>
  );
}

export { SearchInput };
