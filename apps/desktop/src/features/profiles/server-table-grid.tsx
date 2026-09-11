import { navigateVirtualList } from "./virtual-list-keyboard";
import { ChevronRight, Inbox, LoaderCircle } from "lucide-react";
import { NodeCountryIcon } from "@/components/node-country-icon";

import { EmptyState } from "@voya/ui/components/empty-state";
import { Skeleton } from "@voya/ui/components/skeleton";
import { cn } from "@voya/ui/lib/utils";

import { NodeGroupCard } from "./node-group-card";

import { profileLatency } from "./profile-card-data";
import { getProtocolLabel } from "./profile-constants";
import { profileAddress } from "./profile-display";
import { ProfileCardMenu, ProfileRowContextMenu } from "./server-table-menus";
import type { ServerTableController } from "./use-server-table";

export function ProfileCardList({
  controller,
}: {
  controller: ServerTableController;
}) {
  const {
    activation,
    openDetails,
    profilesQuery,
    rows,
    renderedRows,
    rowVirtualizer,
    selectOnly,
    selectedId,
    t,
    viewportRef,
  } = controller;
  return (
    <div className="min-h-0 flex-1 p-4 min-[1100px]:p-page">
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
            aria-label={t("status.loadingScreen")}
            aria-busy="true"
            className="grid gap-3"
            role="status"
          >
            {Array.from({ length: 6 }, (_, index) => (
              <div className="node-card-surface" key={index}>
                <Skeleton className="size-11 shrink-0 rounded-xl" />
                <div className="grid flex-1 gap-2">
                  <Skeleton className="h-3 w-24" />
                  <Skeleton className="h-5 w-3/4" />
                  <Skeleton className="h-3 w-1/2" />
                </div>
                <Skeleton className="h-9 w-24" />
              </div>
            ))}
          </div>
        ) : rows.length === 0 ? (
          <EmptyState
            className="min-h-[18rem] content-center"
            description={t("panes.profiles.emptyDescription")}
            icon={Inbox}
            title={t("panes.profiles.empty")}
          />
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
              const selected = selectedId === id;
              const running = activation.runningId === id;
              const switching = activation.switchingId === id;
              const rawName = profile.remarks || t("panes.profiles.untitled");
              const flag = rawName.match(/\p{Regional_Indicator}{2}/u)?.[0];
              const name = flag
                ? rawName.replace(flag, "").trim() || rawName
                : rawName;
              const address = profileAddress(profile) || "—";
              return (
                <li key={row.key} {...rowProps}>
                  <ProfileRowContextMenu controller={controller} item={item}>
                    <article
                      className={cn(
                        "node-card-surface node-group-surface profile-node-card",
                        selected && "profile-node-card-selected",
                      )}
                      data-testid="server-row"
                      data-selected={selected}
                      onClick={() => selectOnly(id)}
                    >
                      <div aria-hidden="true" className="node-card-icon">
                        <NodeCountryIcon
                          countryCode={item.metrics.countryCode}
                        />
                      </div>
                      <div className="node-card-content">
                        <button
                          aria-label={t("panes.profiles.card.select", {
                            name: rawName,
                          })}
                          aria-pressed={selected}
                          data-row-focus
                          className="node-card-select"
                          onClick={() => selectOnly(id)}
                          type="button"
                        >
                          <span className="node-card-label">
                            {running ? (
                              <span className="node-card-state">
                                <span
                                  aria-hidden="true"
                                  className="size-1.5 rounded-full bg-connected"
                                />
                                {t("panes.profiles.aria.activeProfile")}
                              </span>
                            ) : item.isActive ? (
                              <span className="shrink-0">
                                {t("panes.profiles.card.default")}
                              </span>
                            ) : null}
                          </span>
                          <span className="node-card-name" title={rawName}>
                            {name}
                          </span>
                        </button>
                        <div className="node-card-meta">
                          <span className="node-card-address" title={address}>
                            {address}
                          </span>
                          <span>{getProtocolLabel(profile.protocol.kind)}</span>
                          <button
                            className="node-card-details"
                            onClick={(event) =>
                              openDetails(id, event.currentTarget)
                            }
                            type="button"
                          >
                            {t("panes.profiles.card.details")}
                            <ChevronRight
                              aria-hidden="true"
                              className="size-3"
                            />
                          </button>
                        </div>
                      </div>
                      <div className="profile-node-actions">
                        <span
                          className="profile-node-latency"
                          title={t("panes.profiles.cardFields.delay")}
                        >
                          {profileLatency(item, t)}
                        </span>
                        <button
                          aria-busy={switching || undefined}
                          className="node-card-action"
                          disabled={activation.busy || running}
                          onClick={() => void activation.activateProfile(id)}
                          type="button"
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
                              : t("panes.profiles.card.use")}
                        </button>
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
