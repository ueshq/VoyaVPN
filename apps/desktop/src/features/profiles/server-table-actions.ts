import {
  exportProfileVoyaBundle,
  exportProfileShareLinks,
  exportProfileShareLinksBase64,
} from "@/ipc";
import type { ExportProfilesResult, ImportProfilesResult, ProfileKind } from "@/ipc/bindings";
import { CONFIG_TYPES } from "./profile-constants";
import type { TranslationFunction as TranslateFn } from "@voya/i18n";

export type ProfileExportKind = "shareBase64" | "shareLinks" | "voyaBundle";

// `export_share_link` (crates/voya-core/src/fmt/entry.rs) only knows these node
// protocols and returns `WrongConfigType` for anything else; the backend
// collects the batch with `?`, so a single unsupported profile fails the whole
// export.
const SHARE_LINK_KINDS: readonly ProfileKind[] = [
  CONFIG_TYPES.VMess,
  CONFIG_TYPES.Shadowsocks,
  CONFIG_TYPES.SOCKS,
  CONFIG_TYPES.Trojan,
  CONFIG_TYPES.VLESS,
  CONFIG_TYPES.Hysteria2,
  CONFIG_TYPES.TUIC,
  CONFIG_TYPES.WireGuard,
  CONFIG_TYPES.Anytls,
  CONFIG_TYPES.Naive,
];

export function isShareLinkExport(kind: ProfileExportKind) {
  return kind === "shareLinks" || kind === "shareBase64";
}

export function supportsShareLinkExport(kind: ProfileKind) {
  return SHARE_LINK_KINDS.includes(kind);
}

export function runProfileExport(kind: ProfileExportKind, indexIds: string[]): Promise<ExportProfilesResult> {
  switch (kind) {
    case "voyaBundle":
      return exportProfileVoyaBundle(indexIds);
    case "shareBase64":
      return exportProfileShareLinksBase64(indexIds);
    case "shareLinks":
      return exportProfileShareLinks(indexIds);
  }
}

export function exportFileName(kind: ProfileExportKind) {
  switch (kind) {
    case "voyaBundle":
      return "voyavpn-node-bundle.voya";
    case "shareBase64":
      return "voyavpn-share-links-base64.txt";
    case "shareLinks":
      return "voyavpn-share-links.txt";
  }
}

export function exportFileFilter(kind: ProfileExportKind, t: TranslateFn) {
  return kind === "voyaBundle"
      ? { extensions: ["voya"], name: t("panes.profiles.export.voyaBundle") }
      : { extensions: ["txt"], name: "Text" };
}

/**
 * Single localized summary for an `ImportProfilesResult`, shared by the import
 * dialog and the profiles toolbar banner so one import can never produce two
 * differently worded sentences.
 */
export function formatImportSummary(result: ImportProfilesResult, t: TranslateFn) {
  const parts = [
    t("panes.profiles.import.summary.imported", { count: result.imported.toLocaleString() }),
  ];

  if (result.updated > 0) {
    parts.push(t("panes.profiles.import.summary.updated", { count: result.updated.toLocaleString() }));
  }
  if (result.removedDuplicates > 0) {
    parts.push(
      t("panes.profiles.import.summary.removedDuplicates", {
        count: result.removedDuplicates.toLocaleString(),
      }),
    );
  }
  if (result.skipped > 0) {
    parts.push(t("panes.profiles.import.summary.skipped", { count: result.skipped.toLocaleString() }));
  }
  if (result.failed > 0) {
    parts.push(t("panes.profiles.import.summary.failed", { count: result.failed.toLocaleString() }));
  }
  if (result.filtered > 0) {
    parts.push(t("panes.profiles.import.summary.filtered", { count: result.filtered.toLocaleString() }));
  }
  if (result.deduped > 0) {
    parts.push(t("panes.profiles.import.summary.deduped", { count: result.deduped.toLocaleString() }));
  }
  if (result.discardedNodeOverrides > 0) {
    parts.push(
      t("panes.profiles.import.discardedNodeOverrides", {
        count: result.discardedNodeOverrides.toLocaleString(),
      }),
    );
  }

  return parts.join(" ");
}
