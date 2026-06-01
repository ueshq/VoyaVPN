import * as React from "react";
import * as ToastPrimitive from "@radix-ui/react-toast";
import { X } from "lucide-react";

import { cn } from "@/lib/utils";

const ToastProvider: typeof ToastPrimitive.Provider = ToastPrimitive.Provider;

const ToastViewport = React.forwardRef<
  React.ElementRef<typeof ToastPrimitive.Viewport>,
  React.ComponentPropsWithoutRef<typeof ToastPrimitive.Viewport>
>(({ className, ...props }, ref) => (
  <ToastPrimitive.Viewport
    ref={ref}
    className={cn(
      "fixed bottom-0 end-0 z-[100] flex max-h-screen w-full flex-col-reverse gap-2 p-4 sm:max-w-sm",
      className,
    )}
    {...props}
  />
));
ToastViewport.displayName = ToastPrimitive.Viewport.displayName;

const Toast = React.forwardRef<
  React.ElementRef<typeof ToastPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof ToastPrimitive.Root>
>(({ className, ...props }, ref) => (
  <ToastPrimitive.Root
    ref={ref}
    className={cn(
      "pointer-events-auto relative grid w-full gap-1 overflow-hidden rounded-lg border bg-card p-4 pe-10 text-card-foreground shadow-lg",
      "data-[swipe=move]:translate-x-[var(--radix-toast-swipe-move-x)] data-[swipe=cancel]:translate-x-0",
      "data-[state=closed]:opacity-0 data-[state=open]:opacity-100",
      className,
    )}
    {...props}
  />
));
Toast.displayName = ToastPrimitive.Root.displayName;

const ToastTitle = React.forwardRef<
  React.ElementRef<typeof ToastPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof ToastPrimitive.Title>
>(({ className, ...props }, ref) => (
  <ToastPrimitive.Title ref={ref} className={cn("text-sm font-semibold", className)} {...props} />
));
ToastTitle.displayName = ToastPrimitive.Title.displayName;

const ToastDescription = React.forwardRef<
  React.ElementRef<typeof ToastPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof ToastPrimitive.Description>
>(({ className, ...props }, ref) => (
  <ToastPrimitive.Description ref={ref} className={cn("text-sm text-muted-foreground", className)} {...props} />
));
ToastDescription.displayName = ToastPrimitive.Description.displayName;

const ToastClose = React.forwardRef<
  React.ElementRef<typeof ToastPrimitive.Close>,
  React.ComponentPropsWithoutRef<typeof ToastPrimitive.Close>
>(({ className, ...props }, ref) => (
  <ToastPrimitive.Close
    ref={ref}
    className={cn(
      "absolute end-2 top-2 inline-flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground",
      "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
      className,
    )}
    toast-close=""
    {...props}
  >
    <X className="size-4" aria-hidden="true" />
    <span className="sr-only">Close</span>
  </ToastPrimitive.Close>
));
ToastClose.displayName = ToastPrimitive.Close.displayName;

export { Toast, ToastClose, ToastDescription, ToastProvider, ToastTitle, ToastViewport };
