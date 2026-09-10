export type RuntimeChannel = "coreState" | "sysProxy" | "tun";

const versions = {
  coreState: { request: 0, event: 0 },
  sysProxy: { request: 0, event: 0 },
  tun: { request: 0, event: 0 },
};

/** A newer request or an intervening event supersedes a pending status read. */
export function beginRuntimeRead(channel: RuntimeChannel): () => boolean {
  const version = versions[channel];
  const request = ++version.request;
  const event = version.event;
  return () => version.request === request && version.event === event;
}

export function markRuntimeUpdate(channel: RuntimeChannel): void {
  versions[channel].event += 1;
}
