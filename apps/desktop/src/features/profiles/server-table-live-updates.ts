import type { ProfileListEntry, SpeedtestResult } from "@/ipc/bindings";

export function applySpeedtestResults(
  profiles: ProfileListEntry[],
  speedtestResults: Record<string, SpeedtestResult>,
) {
  if (Object.keys(speedtestResults).length === 0) return profiles;

  let changed = false;
  const nextProfiles = profiles.map((item) => {
    const result = speedtestResults[item.profile.id];
    if (!result) return item;

    changed = true;
    return {
      ...item,
      metrics: {
        ...item.metrics,
        // Country stays on the query snapshot; cached events may outlive edits.
        delayMs: result.delay ?? item.metrics.delayMs,
        ipInfo: result.ipInfo ?? item.metrics.ipInfo,
        outcome: result.outcome,
      },
    };
  });

  return changed ? nextProfiles : profiles;
}
