import type {
  ProfileListItem_Serialize,
  ServerStatItem,
  SpeedTestResult,
} from "@/ipc/bindings";

export function applyLiveUpdates(
  profiles: ProfileListItem_Serialize[],
  liveStats: Record<string, ServerStatItem> = {},
  speedtestResults: Record<string, SpeedTestResult> = {},
) {
  if (Object.keys(liveStats).length === 0 && Object.keys(speedtestResults).length === 0) {
    return profiles;
  }

  let changed = false;
  const nextProfiles = profiles.map((item) => {
    const serverStat = liveStats[item.profile.IndexId];
    const speedtestResult = speedtestResults[item.profile.IndexId];
    const withStats = serverStat ? { ...item, serverStat } : item;

    if (!speedtestResult) {
      changed ||= Boolean(serverStat);
      return withStats;
    }

    changed = true;
    return {
      ...withStats,
      profileEx: {
        ...withStats.profileEx,
        Delay: speedtestResult.delay ?? withStats.profileEx.Delay,
        IpInfo: speedtestResult.ipInfo ?? withStats.profileEx.IpInfo,
        Message: speedtestResult.message ?? withStats.profileEx.Message,
        Speed: speedtestResult.speed ?? withStats.profileEx.Speed,
      },
    };
  });

  return changed ? nextProfiles : profiles;
}
