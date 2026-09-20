import { useState } from "react";
import { voyaCommands } from "@voya/client/transport";
import { clipboard } from "@voya/client/platform";
import { profilesByNodeGroup, type NodeSourceKey } from "@voya/features/profiles/node-list-rows";
import {
  supportsShareLinkExport,
  type ProfileExportDestination,
} from "@voya/features/profiles/server-table-actions";
import type { TranslationFunction } from "@voya/i18n";
import type { NodeOperation } from "@voya/features/profiles/use-node-operation";

export function useNodeExport(
  { runOperation, setOperationError, setOperationMessage }: NodeOperation,
  t: TranslationFunction,
) {
  const [shareQrContent, setShareQrContent] = useState<string | null>(null);
  async function performExport(
    indexIds: string[],
    destination: ProfileExportDestination,
  ) {
    const result = await voyaCommands().exportProfileShareLinks(indexIds);
    if (destination === "qr") {
      setShareQrContent(result.text);
      return;
    }

    await clipboard().writeText(result.text);
    setOperationMessage(
      t("panes.profiles.export.copied", { count: result.count }),
    );
  }

  async function handleExport(
    indexIds: string[],
    destination: ProfileExportDestination = "clipboard",
  ) {
    await runOperation(async () => {
      if (indexIds.length === 0) {
        setOperationError(t("panes.profiles.export.noSelection"));
        return;
      }
      await performExport(indexIds, destination);
    });
  }

  async function handleGroupExport(
    groupKey: NodeSourceKey,
    destination: ProfileExportDestination = "clipboard",
  ) {
    await runOperation(async () => {
      const listing = await voyaCommands().listProfileSummaries();
      const entries = profilesByNodeGroup(listing.entries).get(groupKey) ?? [];
      const exportable = entries.filter((item) =>
        supportsShareLinkExport(item.profile.kind),
      );
      if (!exportable.length) {
        setOperationError(t("panes.profiles.export.noProfiles"));
        return;
      }
      await performExport(
        exportable.map((item) => item.profile.id),
        destination,
      );
      const skipped = entries.length - exportable.length;
      if (skipped > 0)
        setOperationMessage(
          t("panes.profiles.export.skippedUnsupported", { count: skipped }),
        );
    });
  }

  return {
    handleExport,
    handleGroupExport,
    shareQrContent,
    setShareQrContent,
  };
}
