import { navigateVirtualList } from "./virtual-list-keyboard";
import { Inbox, Plus } from "lucide-react";
import { Button } from "@voya/ui/components/button";
import { useShellStore } from "@/stores/shell-store";
import { NodeCountryIcon } from "@/components/node-country-icon";
import { PageSurface } from "@/components/app-shell/page-section";
import {
  dataTableRowHover,
  dataTableRowSelected,
} from "@/components/app-shell/data-table-surface";

import { EmptyState } from "@voya/ui/components/empty-state";
import { Skeleton } from "@voya/ui/components/skeleton";
import { Spinner } from "@voya/ui/components/spinner";
import { cn } from "@voya/ui/lib/utils";

import { NodeGroupCard } from "./node-group-card";

import {
  entryCountry,
  profileLatency,
  profileLatencyTone,
  profileNameWithoutFlag,
  profileTitle,
} from "./profile-display";
import { getProtocolLabel } from "./profile-constants";

import { ProfileCardMenu, ProfileRowContextMenu } from "./server-table-menus";
import { overlaySpeedtestResult } from "./use-node-list-data";
import type { ServerTableController } from "./use-server-table";

export function ProfileCardList({
  controller,
}: {
  controller: ServerTableController;
}) {
  const {
    activation,
    nodeGroups,
    openDetails,
    profilesQuery,
    rows,
    renderedRows,
    rowVirtualizer,
    speedtestResultsByProfileId,
    t,
    viewportRef,
  } = controller;
  return (
    <div className="min-h-0 min-w-0 flex-1">
      <div
        aria-label={t("panes.profiles.title")}
        className="profile-card-list h-full overflow-y-auto outline-none focus-visible:ring-2 focus-visible:ring-ring"
        data-testid="server-table-viewport"
        ref={viewportRef}
        tabIndex={0}
        onKeyDown={(event) =>
          navigateVirtualList(
            event,
            rows.length,
            (index) => rowVirtualizer.scrollToIndex(index),
            "data-index",
            "button[data-row-focus]",
          )
        }
      >
        {profilesQuery.isLoading && !rows.length ? (
          <div
            aria-label={t("panes.profiles.loadingNodes")}
            aria-busy="true"
            className="grid gap-2"
            role="status"
          >
            {Array.from({ length: 8 }, (_, index) => (
              <div className="node-card-surface profile-node-card" key={index}>
                <Skeleton className="size-8 shrink-0 rounded-lg" />
                <div className="grid flex-1 gap-2">
                  <Skeleton className="h-4 w-1/2" />
                  <Skeleton className="h-3 w-1/3" />
                </div>
                <Skeleton className="h-8 w-20" />
              </div>
            ))}
          </div>
        ) : rows.length === 0 ? (
          <PageSurface className="h-full">
            <EmptyState
              actions={
                <Button
                  onClick={() => nodeGroups.search.trim() ? nodeGroups.clearSearch() : useShellStore.getState().openProfilesAddMenu()}
                  size="sm"
                  type="button"
                >
                  {nodeGroups.search.trim() ? null : <Plus aria-hidden="true" className="size-4" />}
                  {t(nodeGroups.search.trim() ? "panes.profiles.search.clear" : "panes.profiles.toolbar.addNode")}
                </Button>
              }
              className="h-full content-center"
              description={t(nodeGroups.search.trim() ? "panes.profiles.search.emptyHint" : "panes.profiles.emptyDescription")}
              icon={Inbox}
              title={t(nodeGroups.search.trim() ? "panes.profiles.search.empty" : "panes.profiles.empty")}
            />
          </PageSurface>
        ) : (
          <ul
            aria-label={t("panes.profiles.title")}
            className="relative w-full"
            style={{ height: rowVirtualizer.getTotalSize() }}
          >
            {renderedRows.map((virtualRow) => {
              const row = rows[virtualRow.index];
              if (!row) return null;
              const rowProps = {
                "aria-posinset": virtualRow.index + 1,
                "aria-setsize": rows.length,
                className: cn(
                  "absolute start-0 top-0 w-full node-group-segment",
                  row.kind === "group" && "node-group-start",
                  row.last && "node-group-end",
                ),
                "data-group-key": row.groupKey,
                "data-index": virtualRow.index,
                ref: rowVirtualizer.measureElement,
                style: { transform: `translateY(${virtualRow.start}px)` },
              };
              if (row.kind === "group")
                return (
                  <li key={row.key} {...rowProps}>
                    <NodeGroupCard row={row} controller={controller} />
                  </li>
                );
              // Rows are laid out from the listing while nothing sorts or
              // filters by latency; live results land on the rendered few.
              const item = overlaySpeedtestResult(
                row.item,
                speedtestResultsByProfileId[row.item.profile.id],
              );
              const { profile } = item;
              const id = profile.id;
              const running = activation.runningId === id;
              const switching = activation.switchingId === id;
              const rawName = profileTitle(profile.remarks, t);
              const name = profileNameWithoutFlag(rawName);
              const address = profile.address || "—";
              const tone = profileLatencyTone(item);
              return (
                <li key={row.key} {...rowProps}>
                  {/* The wrapper paints the group panel; the row itself carries
                      only the shared table row colors, which a panel background
                      on the same element would override. */}
                  <div className="node-group-surface">
                    <ProfileRowContextMenu controller={controller} item={item}>
                      <article
                        className={cn(
                          "profile-node-card",
                          // The highlight marks the node a connection uses; it
                          // keeps its color under the pointer.
                          item.isActive ? dataTableRowSelected : dataTableRowHover,
                          item.isActive && "profile-node-card-selected",
                        )}
                        data-testid="server-row"
                      >
                        <div aria-hidden="true" className="node-card-icon">
                          {/* A measured exit wins; a flag in the name is the provisional hint. */}
                          <NodeCountryIcon countryCode={entryCountry(item)} />
                        </div>
                        <div className="node-card-content">
                          {/* The name opens the node's details, where it can also be used or tested. */}
                          <button
                            aria-label={t("panes.profiles.card.openDetails", {
                              name: rawName,
                            })}
                            data-row-focus
                            title={t("panes.profiles.card.openDetails", { name: rawName })}
                            className="node-card-select"
                            onClick={(event) => openDetails(id, event.currentTarget)}
                            type="button"
                          >
                            <span className="node-card-title">
                              <span className="node-card-name" title={rawName}>
                                {name}
                              </span>
                              {running ? (
                                <span className="node-card-state" data-state="running">
                                  <span
                                    aria-hidden="true"
                                    className="size-1.5 rounded-full bg-connected"
                                  />
                                  {t("panes.profiles.aria.activeProfile")}
                                </span>
                              ) : item.isActive ? (
                                <span className="node-card-state">
                                  {t("panes.profiles.card.default")}
                                </span>
                              ) : null}
                            </span>
                          </button>
                          <div className="node-card-meta">
                            <span className="node-card-address" title={address}>
                              {address}
                            </span>
                            <span>{getProtocolLabel(profile.kind)}</span>
                          </div>
                        </div>
                        <div className="profile-node-actions">
                          <span
                            className="profile-node-latency"
                            data-tone={tone}
                            title={t("panes.profiles.cardFields.delay")}
                          >
                            {tone === "unknown" ? null : (
                              <span aria-hidden="true" className="profile-node-latency-dot" />
                            )}
                            {profileLatency(item, t)}
                          </span>
                          <Button
                            aria-busy={switching || undefined}
                            disabled={activation.busy || running}
                            onClick={() => void activation.activateProfile(id)}
                            size="sm"
                            type="button"
                            variant="outline"
                          >
                            {switching ? (
                              <Spinner className="size-4" />
                            ) : null}
                            {switching
                              ? t("panes.profiles.card.switching")
                              : running
                                ? t("panes.profiles.card.using")
                                : t(activation.runningId ? "panes.profiles.card.switch" : "panes.profiles.card.connect")}
                          </Button>
                          <ProfileCardMenu controller={controller} item={item} />
                        </div>
                      </article>
                    </ProfileRowContextMenu>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
