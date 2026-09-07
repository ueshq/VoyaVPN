import type {
  ProfileListEntry,
  ServerStatItem,
  SpeedTestResult,
} from "@/ipc/bindings";

export function applyLiveUpdates(
  profiles: ProfileListEntry[],
  liveStats: Record<string, ServerStatItem> = {},
  speedtestResults: Record<string, SpeedTestResult> = {},
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
        delayMs: speedtestResult.delay ?? withStats.metrics.delayMs,
        ipInfo: speedtestResult.ipInfo ?? withStats.metrics.ipInfo,
        outcome: speedtestResult.outcome,
        speedBytesPerSecond: speedtestResult.speed ?? withStats.metrics.speedBytesPerSecond,
      },
    };
  });

  return changed ? nextProfiles : profiles;
}
