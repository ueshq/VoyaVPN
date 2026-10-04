import { redactOperationalError } from "@voya/utils/operational-redaction";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { voyaCommands } from "@voya/client/transport";
import type { ResourceUpdateFile } from "@voya/contracts";
import { usePreferencesStore } from "@voya/client/preferences-store";

import { saveQueue } from "../forms/save-queue";
import { useBusyAction } from "../forms/use-busy-action";

/**
 * Refreshing the rule library: the `.srs` rule sets the routing rules name.
 *
 * Shared because both shells offer it and neither owns it — the desktop from
 * its update dialog, a phone from a Settings row. What is *not* shared is the
 * app's own self-update beside it on the desktop: an App Store or Play build
 * is updated by the store, so a phone has no equivalent and must not grow one.
 */
export function useRuleLibraryUpdate() {
  const queue = saveQueue(useQueryClient());
  const [files, setFiles] = useState<ResourceUpdateFile[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { busy: updating, run } = useBusyAction();
  const updatedAt = usePreferencesStore((state) => state.ruleLibraryUpdatedAt);

  /**
   * The backend publishes the batch all or nothing, so a failure shows no
   * files. One at a time: a second press before the button disables would run
   * two downloads into the same directory.
   */
  function update() {
    return run(async () => {
      setError(null);
      setFiles(null);
      try {
        // A queued settings write would otherwise land on top of the assets
        // this is about to replace.
        await queue.settled();
        setFiles(await voyaCommands().updateSrsAssets());
        usePreferencesStore.getState().setRuleLibraryUpdatedAt(Date.now());
      } catch (failure) {
        setFiles([]);
        setError(redactOperationalError(failure));
      }
    });
  }

  return { error, files, update, updatedAt, updating };
}
