import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import { voyaCommands } from "@voya/client/transport";
import type { ResourceUpdateFile } from "@voya/contracts";
import { usePreferencesStore } from "@voya/client/preferences-store";
import { getErrorMessage } from "@voya/utils/error";

import { saveQueue } from "../forms/save-queue";

/**
 * Refreshing the rule library: the IP and domain data, then the rule sets.
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
  const [updating, setUpdating] = useState(false);
  const updatedAt = usePreferencesStore((state) => state.ruleLibraryUpdatedAt);

  /** A failure part way keeps the files that did arrive on show. */
  async function update() {
    const arrived: ResourceUpdateFile[] = [];
    setUpdating(true);
    setError(null);
    setFiles(null);
    try {
      // A queued settings write would otherwise land on top of the assets
      // this is about to replace.
      await queue.settled();
      arrived.push(...(await voyaCommands().updateGeoAssets()));
      arrived.push(...(await voyaCommands().updateSrsAssets()));
      usePreferencesStore.getState().setRuleLibraryUpdatedAt(Date.now());
    } catch (failure) {
      setError(getErrorMessage(failure));
    } finally {
      setFiles(arrived);
      setUpdating(false);
    }
  }

  return { error, files, update, updatedAt, updating };
}
