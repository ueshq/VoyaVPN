import { useState } from "react";
import { getErrorMessage } from "@voya/utils/error";

export function useNodeOperation() {
  const [operationError, setOperationError] = useState<string | null>(null);
  const [operationMessage, setOperationMessage] = useState<string | null>(null);
  // Reports whether the operation succeeded so callers that own a dialog can
  // keep it open (with the user's edits) when the backend rejects the request.
  // `onError` redirects the message to that dialog instead of the toolbar
  // banner, which a modal would cover.
  async function runOperation(
    operation: () => Promise<unknown>,
    onError: (message: string) => void = setOperationError,
  ) {
    setOperationError(null);
    setOperationMessage(null);
    try {
      // Mutating commands emit their own cache invalidations.
      await operation();
      return true;
    } catch (error) {
      onError(getErrorMessage(error));
      return false;
    }
  }

  return {
    operationError,
    operationMessage,
    setOperationError,
    setOperationMessage,
    runOperation,
  };
}

export type NodeOperation = ReturnType<typeof useNodeOperation>;
