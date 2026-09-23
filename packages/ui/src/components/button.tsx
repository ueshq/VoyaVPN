import * as React from "react";

import { buttonVariants, type ButtonVariantProps } from "@voya/ui/components/button-variants";
import { cn } from "@voya/ui/lib/utils";

type ButtonProps = React.ComponentPropsWithRef<"button"> & ButtonVariantProps;

function Button({ className, variant, size, ...props }: ButtonProps) {
  return <button data-slot="button" className={cn(buttonVariants({ variant, size, className }))} {...props} />;
}

export { Button };
