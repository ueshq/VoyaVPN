import { Suspense, lazy, useState } from "react";

import { useRuntimeActionStore } from "@voya/client/runtime-action-store";
import { useShellStore } from "@/stores/shell-store";

// The shell's two dialogs are the only startup code that needs the dialog
// primitives, the checkbox and the submit helper, and neither shows at
// startup — so both load when they are first asked for.
const MissingCoreDialog = lazy(() =>
  import("./missing-core-dialog").then(({ MissingCoreDialog }) => ({ default: MissingCoreDialog })),
);
const CloseRequestDialog = lazy(() =>
  import("./close-request-dialog").then(({ CloseRequestDialog }) => ({
    default: CloseRequestDialog,
  })),
);

export function ModalHost() {
  const missingCore = useRuntimeActionStore((state) => state.missingCore);
  const closeRequested = useShellStore((state) => state.closeRequestOpen);
  // Once asked for, the close prompt stays mounted and opens and closes
  // itself, so it keeps its closing transition.
  const [closePromptLoaded, setClosePromptLoaded] = useState(closeRequested);
  if (closeRequested && !closePromptLoaded) setClosePromptLoaded(true);

  return (
    <Suspense fallback={null}>
      {missingCore ? <MissingCoreDialog /> : null}
      {closePromptLoaded ? <CloseRequestDialog /> : null}
    </Suspense>
  );
}
