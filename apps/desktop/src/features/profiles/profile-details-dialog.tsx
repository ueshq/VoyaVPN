import { Fragment } from "react";
import type { TranslationKey } from "@voya/i18n";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@voya/ui/components/dialog";
import { formatBytes } from "@voya/utils/formatting";
import { useRuntimeEventStore } from "@/ipc";
import type { ProfileListEntry } from "@/ipc/bindings";

import { profileLatency } from "./profile-card-data";
import { getProtocolLabel } from "./profile-constants";
import { profileAddress, profilePort, profileTransportName } from "./profile-display";
import type { ServerTableController } from "./use-server-table";

// Mounted only while open; the statistics selector watches this node alone.
export function ProfileDetailsDialog({ controller, item }: { controller: ServerTableController; item: ProfileListEntry }) {
  const { t, setDetailsId, restoreDetailsFocus, subscriptionName } = controller;
  const stat = useRuntimeEventStore((state) => state.serverStatsByProfileId[item.profile.id]);
  const { profile, traffic } = item;
  const rows: [TranslationKey, string | number][] = [
    ["panes.profiles.cardFields.remarks", profile.remarks || t("panes.profiles.untitled")],
    ["panes.profiles.cardFields.address", profileAddress(profile) || "—"],
    ["panes.profiles.cardFields.port", profilePort(profile) || "—"],
    ["panes.profiles.cardFields.protocol", getProtocolLabel(profile.protocol.kind)],
    ["panes.profiles.cardFields.group", subscriptionName(item)],
    ["panes.profiles.cardFields.transport", profile.transport ? profileTransportName(profile.transport) : "—"],
    ["panes.profiles.cardFields.security", profile.tls?.mode ?? "—"],
    ["panes.profiles.cardFields.delay", profileLatency(item, t)],
    ["panes.profiles.cardFields.ipInfo", item.metrics.ipInfo || "—"],
    ["panes.profiles.cardFields.todayUp", formatBytes(stat?.todayUp ?? traffic.todayUpload)],
    ["panes.profiles.cardFields.todayDown", formatBytes(stat?.todayDown ?? traffic.todayDownload)],
    ["panes.profiles.cardFields.totalUp", formatBytes(stat?.totalUp ?? traffic.totalUpload)],
    ["panes.profiles.cardFields.totalDown", formatBytes(stat?.totalDown ?? traffic.totalDownload)],
  ];
  return (
    <Dialog open onOpenChange={(open) => { if (!open) setDetailsId(null); }}>
      <DialogContent className="max-h-[90vh] overflow-y-auto" closeLabel={t("actions.close")} onCloseAutoFocus={(event) => { event.preventDefault(); restoreDetailsFocus(); }}>
        <DialogHeader>
          <DialogTitle>{t("panes.profiles.card.detailsTitle")}</DialogTitle>
          <DialogDescription className="break-all">{profile.remarks || t("panes.profiles.untitled")}</DialogDescription>
        </DialogHeader>
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-4 p-6 text-sm [&_dt]:text-muted-foreground [&_dd]:break-all">
          {rows.map(([label, value]) => <Fragment key={label}><dt>{t(label)}</dt><dd>{value}</dd></Fragment>)}
        </dl>
      </DialogContent>
    </Dialog>
  );
}
