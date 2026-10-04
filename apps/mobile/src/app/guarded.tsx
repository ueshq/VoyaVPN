import type { ComponentType } from "react";

import { ScreenErrorBoundary } from "./screen-error-boundary";

/** `Screen` inside a boundary of its own. Call once per screen, at module scope. */
export function guarded<Props extends object>(Screen: ComponentType<Props>): ComponentType<Props> {
  return function GuardedScreen(props: Props) {
    return (
      <ScreenErrorBoundary>
        <Screen {...props} />
      </ScreenErrorBoundary>
    );
  };
}
