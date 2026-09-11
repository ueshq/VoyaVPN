import type {
  ProfileListEntry,
  ServerStatItem,
  SpeedtestResult,
} from "@/ipc/bindings";

export function applyLiveUpdates(
  profiles: ProfileListEntry[],
  liveStats: Record<string, ServerStatItem> = {},
  speedtestResults: Record<string, SpeedtestResult> = {},
) {
  if (Object.keys(liveStats).length === 0 && Object.keys(speedtestResults).length === 0) {
    return profiles;
  }

  let changed = false;
  const nextProfiles = profiles.map((item) => {
    const serverStat = liveStats[item.profile.id];
    const speedtestResult = speedtestResults[item.profile.id];
    const withStats = serverStat ? {
      ...item,
      traffic: {
        date: serverStat.dateNow ?? 0,
        todayDownload: serverStat.todayDown ?? 0,
        todayUpload: serverStat.todayUp ?? 0,
        totalDownload: serverStat.totalDown ?? 0,
        totalUpload: serverStat.totalUp ?? 0,
      },
    } : item;

    if (!speedtestResult) {
      changed ||= Boolean(serverStat);
      return withStats;
    }

    changed = true;
    return {
      ...withStats,
      metrics: {
        ...withStats.metrics,
        // countryCode deliberately stays on the authoritative query snapshot.
        // EventBridge refreshes it after results; cached events may outlive edits.
        delayMs: speedtestResult.delay ?? withStats.metrics.delayMs,
        ipInfo: speedtestResult.ipInfo ?? withStats.metrics.ipInfo,
        outcome: speedtestResult.outcome,
      },
    };
  });

  return changed ? nextProfiles : profiles;
}
