import { useState } from "react";

import type { TranslationFunction } from "@voya/i18n/core";
import { redactOperationalError } from "@voya/utils/operational-redaction";
import { appErrorOfKind } from "@voya/client/errors";
import { validationFieldErrors } from "@voya/client/messages";

/**
 * The pending flag and failure of a dialog's save. `submit` clears the last
 * failure and runs the save; a rejection becomes a redacted message. A dialog
 * that shows backend validation issues next to its fields passes `t`, and
 * those issues land in `fieldErrors` instead.
 */
export function useDialogSubmit(fieldErrorsT?: TranslationFunction) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});

  async function submit(save: () => Promise<void>) {
    setPending(true);
    setError(null);
    setFieldErrors({});
    try {
      await save();
    } catch (cause) {
      const validation = fieldErrorsT ? appErrorOfKind(cause, "validation") : null;
      if (fieldErrorsT && validation) {
        setFieldErrors(validationFieldErrors(fieldErrorsT, validation.kind.issues));
      } else {
        setError(redactOperationalError(cause));
      }
    } finally {
      setPending(false);
    }
  }

  return { error, fieldErrors, pending, setError, submit };
}
