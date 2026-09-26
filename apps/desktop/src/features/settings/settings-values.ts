/**
 * What the backend seeds for each required number (voya-contracts
 * `settings.rs`). A cleared field restores it instead of inventing a value.
 */
export const SETTING_DEFAULTS = {
  fragmentFallbackDelayMs: 500,
  hysteriaHopIntervalSeconds: 30,
  hysteriaMbps: 100,
  localPort: 10_808,
  muxMaxConnections: 8,
  speedTestTimeoutSeconds: 10,
  tunMtu: 1500,
} as const;
