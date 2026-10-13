import type { ComponentProps } from "react";

/** A static anchor with the shared action geometry; no client runtime. */
export function ActionLink({
  variant = "primary",
  className = "",
  ...props
}: ComponentProps<"a"> & { variant?: "primary" | "secondary" }) {
  return <a {...props} className={`site-action site-action-${variant} ${className}`} />;
}
