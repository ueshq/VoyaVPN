import { useState } from "react";
import { listProfiles } from "@/ipc/commands";
import { saveTextFile } from "@/ipc/file-dialog";
import type { ProfileListEntry } from "@/ipc/bindings";
import { profilesByNodeGroup, type NodeSourceKey } from "./node-list-rows";
import {
  exportFileFilter,
  exportFileName,
  isShareLinkExport,
  runProfileExport,
  supportsShareLinkExport,
  type ProfileExportKind,
  type ProfileExportDestination,
} from "./server-table-actions";
import type { TranslationFunction } from "@voya/i18n";
import type { NodeOperation } from "./use-node-operation";

export function useNodeExport(
  { runOperation, setOperationError, setOperationMessage }: NodeOperation,
  t: TranslationFunction,
) {
  const [shareQrContent, setShareQrContent] = useState<string | null>(null);
  async function performExport(
    kind: ProfileExportKind,
    indexIds: string[],
    destination: ProfileExportDestination,
  ) {
    const result = await runProfileExport(kind, indexIds);
    if (destination === "qr") {
      setShareQrContent(result.text);
      return true;
    }

    if (destination === "file") {
      const path = await saveTextFile({
        defaultPath: exportFileName(kind),
        filters: [exportFileFilter(kind, t)],
        text: result.text,
      });
      if (path) {
        setOperationMessage(t("panes.profiles.export.savedFile", { path }));
      }
      return !!path;
    }

    if (!navigator.clipboard?.writeText) {
      throw new Error(t("panes.profiles.export.clipboardUnavailable"));
    }
    await navigator.clipboard.writeText(result.text);
    setOperationMessage(
      t("panes.profiles.export.copied", { count: result.count }),
    );
    return true;
  }

  async function handleExport(
    kind: ProfileExportKind,
    indexIds: string[],
    destination: ProfileExportDestination = "clipboard",
  ) {
    await runOperation(async () => {
      if (indexIds.length === 0) {
        setOperationError(t("panes.profiles.export.noSelection"));
        return;
      }
      await performExport(kind, indexIds, destination);
    });
  }

  async function handleBulkExport(
    kind: ProfileExportKind,
    destination: ProfileExportDestination = "clipboard",
  ) {
    await runOperation(async () => {
      const allProfiles = (await listProfiles(null, null)).entries;
      await performBatchExport(kind, allProfiles, destination);
    });
  }

  async function performBatchExport(
    kind: ProfileExportKind,
    entries: ProfileListEntry[],
    destination: ProfileExportDestination,
  ) {
    const exportable = isShareLinkExport(kind)
      ? entries.filter((item) =>
          supportsShareLinkExport(item.profile.protocol.kind),
        )
      : entries;
    if (!exportable.length) {
      setOperationError(t("panes.profiles.export.noProfiles"));
      return;
    }
    const completed = await performExport(
      kind,
      exportable.map((item) => item.profile.id),
      destination,
    );
    const skipped = entries.length - exportable.length;
    if (completed && skipped > 0)
      setOperationMessage(
        t("panes.profiles.export.skippedUnsupported", { count: skipped }),
      );
  }

  async function handleGroupExport(
    groupKey: NodeSourceKey,
    kind: ProfileExportKind,
    destination: ProfileExportDestination = "clipboard",
  ) {
    await runOperation(async () => {
      const listing = await listProfiles(null, null);
      await performBatchExport(
        kind,
        profilesByNodeGroup(listing.entries).get(groupKey) ?? [],
        destination,
      );
    });
  }

  return {
    handleExport,
    handleBulkExport,
    handleGroupExport,
    shareQrContent,
    setShareQrContent,
  };
}
