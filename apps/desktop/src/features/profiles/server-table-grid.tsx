import { navigateVirtualList } from "./virtual-list-keyboard";
import { Inbox, LoaderCircle, Plus } from "lucide-react";
import { Button } from "@voya/ui/components/button";
import { useShellStore } from "@/stores/shell-store";
import { NodeCountryIcon } from "@/components/node-country-icon";
import { PageSurface } from "@/components/app-shell/page-section";

import { EmptyState } from "@voya/ui/components/empty-state";
import { Skeleton } from "@voya/ui/components/skeleton";
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
import type { NodeListController } from "./node-controller-types";

export function ProfileCardList({
  controller,
}: {
  controller: NodeListController;
}) {
  const {
    activation,
    openDetails,
    profilesQuery,
    rows,
    renderedRows,
    rowVirtualizer,
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
                  onClick={() => useShellStore.getState().openProfilesAddMenu()}
                  size="sm"
                  type="button"
                >
                  <Plus aria-hidden="true" className="size-4" />
                  {t("panes.profiles.toolbar.addNode")}
                </Button>
              }
              className="h-full content-center"
              description={t("panes.profiles.emptyDescription")}
              icon={Inbox}
              title={t("panes.profiles.empty")}
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
              const item = row.item;
              const { profile } = item;
              const id = profile.id;
              const running = activation.runningId === id;
              const switching = activation.switchingId === id;
              const rawName = profileTitle(profile.remarks, t);
              const name = profileNameWithoutFlag(rawName);
              const address = profile.protocol.server.address || "—";
              const tone = profileLatencyTone(item);
              // The chosen, running or switching node keeps its action in view;
              // other rows reveal theirs where the pointer or keyboard focus is.
              const actionPinned = running || switching || item.isActive;
              return (
                <li key={row.key} {...rowProps}>
                  <ProfileRowContextMenu controller={controller} item={item}>
                    <article
                      className={cn(
                        "node-card-surface node-group-surface profile-node-card",
                        // The highlight marks the node a connection uses.
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
                          <span>{getProtocolLabel(profile.protocol.kind)}</span>
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
                          className={cn(!actionPinned && "profile-node-action-idle")}
                          disabled={activation.busy || running}
                          onClick={() => void activation.activateProfile(id)}
                          size="sm"
                          type="button"
                          variant="outline"
                        >
                          {switching ? (
                            <LoaderCircle
                              aria-hidden="true"
                              className="size-4 animate-spin"
                            />
                          ) : null}
                          {switching
                            ? t("panes.profiles.card.switching")
                            : running
                              ? t("panes.profiles.card.using")
                              : item.isActive && !activation.runningId
                                ? // The chosen node while disconnected: this button connects.
                                  t("panes.profiles.card.connect")
                                : t("panes.profiles.card.use")}
                        </Button>
                        <ProfileCardMenu controller={controller} item={item} />
                      </div>
                    </article>
                  </ProfileRowContextMenu>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>
  );
}
