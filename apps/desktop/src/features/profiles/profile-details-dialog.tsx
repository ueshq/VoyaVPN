import { Fragment } from "react";
import { useQuery } from "@tanstack/react-query";
import { Zap } from "lucide-react";
import type { TranslationKey } from "@voya/i18n";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { formatBytes } from "@voya/utils/formatting";
import { getProfile } from "@/ipc/commands";
import { profileDetailsQueryKey } from "@voya/client/query-keys";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { ProfileSummaryEntry } from "@/ipc/bindings";

import { getProtocolLabel } from "@voya/features/profiles/profile-constants";
import { profileLatency, profileTitle, profileTransportName } from "@voya/features/profiles/profile-display";
import type { ServerTableController } from "./use-server-table";

// Mounted only while open; the statistics selector watches this node alone.
// The list carries summaries, so transport, security and stored traffic are
// read in full here and show "—" for the moment that takes.
export function ProfileDetailsDialog({
  controller,
  item,
}: {
  controller: ServerTableController;
  item: ProfileSummaryEntry;
}) {
  const {
    activation,
    handleSpeedtest,
    restoreDetailsFocus,
    setDetailsId,
    speedtestRunning,
    subscriptionName,
    t,
  } = controller;
  const stat = useRuntimeEventStore(
    (state) => state.serverStatsByProfileId[item.profile.id],
  );
  const { profile } = item;
  const details = useQuery({
    queryFn: () => getProfile(profile.id),
    queryKey: profileDetailsQueryKey(profile.id),
  }).data;
  const transport = details?.profile.transport;
  const traffic = details?.traffic;
  const bytes = (live: number | null | undefined, stored: number | null | undefined) => {
    const value = live ?? stored;
    return value == null ? "—" : formatBytes(value);
  };
  const running = activation.runningId === profile.id;
  const rows: [TranslationKey, string | number][] = [
    ["panes.profiles.cardFields.remarks", profileTitle(profile.remarks, t)],
    ["panes.profiles.cardFields.address", profile.address || "—"],
    ["panes.profiles.cardFields.port", profile.port || "—"],
    ["panes.profiles.cardFields.protocol", getProtocolLabel(profile.kind)],
    ["panes.profiles.cardFields.group", subscriptionName(item)],
    [
      "panes.profiles.cardFields.transport",
      transport ? profileTransportName(transport) : "—",
    ],
    ["panes.profiles.cardFields.security", details?.profile.tls?.mode ?? "—"],
    ["panes.profiles.cardFields.delay", profileLatency(item, t)],
    ["panes.profiles.cardFields.ipInfo", item.metrics.ipInfo || "—"],
    ["panes.profiles.cardFields.todayUp", bytes(stat?.todayUp, traffic?.todayUpload)],
    ["panes.profiles.cardFields.todayDown", bytes(stat?.todayDown, traffic?.todayDownload)],
    ["panes.profiles.cardFields.totalUp", bytes(stat?.totalUp, traffic?.totalUpload)],
    ["panes.profiles.cardFields.totalDown", bytes(stat?.totalDown, traffic?.totalDownload)],
  ];
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) setDetailsId(null);
      }}
    >
      <ScrollableDialogContent
        height="viewport"
        width="lg"
        closeLabel={t("actions.close")}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          restoreDetailsFocus();
        }}
      >
        <DialogHeader>
          <DialogTitle>{t("panes.profiles.card.detailsTitle")}</DialogTitle>
          <DialogDescription className="break-all">
            {profileTitle(profile.remarks, t)}
          </DialogDescription>
        </DialogHeader>
        <DialogBody>
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-4 text-sm [&_dt]:text-muted-foreground [&_dd]:break-all">
            {rows.map(([label, value]) => (
              <Fragment key={label}>
                <dt>{t(label)}</dt>
                <dd>{value}</dd>
              </Fragment>
            ))}
          </dl>
        </DialogBody>
        {/* Details should not be a dead end: the two things done with a node are here too. */}
        <DialogFooter>
          <Button
            disabled={speedtestRunning}
            onClick={() =>
              void handleSpeedtest(
                { profileIds: [profile.id], scope: "profiles" },
                `node:${profile.id}`,
              )
            }
            type="button"
            variant="outline"
          >
            <Zap aria-hidden="true" className="size-4" />
            {t("panes.profiles.menu.speedtest")}
          </Button>
          <Button
            disabled={activation.busy || running}
            onClick={() => void activation.activateProfile(profile.id)}
            type="button"
          >
            {running ? t("panes.profiles.card.using") : t("panes.profiles.card.use")}
          </Button>
        </DialogFooter>
      </ScrollableDialogContent>
    </Dialog>
  );
}
