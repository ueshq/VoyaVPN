import type {
  SelfHostAddressKind,
  SelfHostEnvironmentReport,
  SelfHostProblem,
  SelfHostReachability,
  SelfHostReasonCode,
  SelfHostRuntimeStatus,
} from "@voya/contracts";
import type { TranslationKey } from "@voya/i18n";

/**
 * Backend codes of the Self-hosted node page, each to the key that spells it
 * out. `satisfies Record<…>` fails the typecheck when Rust gains a code.
 */
export const STATUS_KEYS = {
  failed: "panes.selfHost.status.failed",
  retrying: "panes.selfHost.status.retrying",
  running: "panes.selfHost.status.running",
  starting: "panes.selfHost.status.starting",
  stopped: "panes.selfHost.status.stopped",
} as const satisfies Record<SelfHostRuntimeStatus, TranslationKey>;

/** What the status means for other devices; a node in transition says nothing. */
export const STATUS_HINT_KEYS = {
  running: "panes.selfHost.statusHint.running",
  stopped: "panes.selfHost.statusHint.stopped",
} as const satisfies Partial<Record<SelfHostRuntimeStatus, TranslationKey>>;

export const PROBLEM_KEYS = {
  coreExited: "panes.selfHost.problem.coreExited",
  coreMissing: "panes.selfHost.problem.coreMissing",
  noProtocol: "panes.selfHost.problem.noProtocol",
  portInUse: "panes.selfHost.problem.portInUse",
  startFailed: "panes.selfHost.problem.startFailed",
} as const satisfies Record<SelfHostProblem, TranslationKey>;

export const ADDRESS_KIND_KEYS = {
  custom: "panes.selfHost.links.addressKind.custom",
  ipv4: "panes.selfHost.links.addressKind.ipv4",
  ipv6: "panes.selfHost.links.addressKind.ipv6",
} as const satisfies Record<SelfHostAddressKind, TranslationKey>;

export const REACHABILITY_KEYS = {
  likelyReachable: "panes.selfHost.network.reachability.likelyReachable",
  needsPortForward: "panes.selfHost.network.reachability.needsPortForward",
  noConnectivity: "panes.selfHost.network.reachability.noConnectivity",
  reachable: "panes.selfHost.network.reachability.reachable",
  unknown: "panes.selfHost.network.reachability.unknown",
  unreachable: "panes.selfHost.network.reachability.unreachable",
} as const satisfies Record<SelfHostReachability, TranslationKey>;

/** The network check in one sentence: can another device connect or not. */
export const VERDICT_KEYS = {
  likelyReachable: "panes.selfHost.network.verdict.likelyReachable",
  needsPortForward: "panes.selfHost.network.verdict.needsPortForward",
  noConnectivity: "panes.selfHost.network.verdict.noConnectivity",
  reachable: "panes.selfHost.network.verdict.reachable",
  unknown: "panes.selfHost.network.verdict.unknown",
  unreachable: "panes.selfHost.network.verdict.unreachable",
} as const satisfies Record<SelfHostReachability, TranslationKey>;

export const REASON_KEYS = {
  behindNat: "panes.selfHost.network.reasons.behindNat",
  carrierGradeNat: "panes.selfHost.network.reasons.carrierGradeNat",
  doubleNat: "panes.selfHost.network.reasons.doubleNat",
  firewallRuleMissing: "panes.selfHost.network.reasons.firewallRuleMissing",
  ipv6FirewallUnknown: "panes.selfHost.network.reasons.ipv6FirewallUnknown",
  noPublicAddress: "panes.selfHost.network.reasons.noPublicAddress",
  portMapped: "panes.selfHost.network.reasons.portMapped",
  probeReachable: "panes.selfHost.network.reasons.probeReachable",
  probeRefused: "panes.selfHost.network.reasons.probeRefused",
  probeTimedOut: "panes.selfHost.network.reasons.probeTimedOut",
  probeUnavailable: "panes.selfHost.network.reasons.probeUnavailable",
  publicAddressOnDevice: "panes.selfHost.network.reasons.publicAddressOnDevice",
  tunActive: "panes.selfHost.network.reasons.tunActive",
  upnpFailed: "panes.selfHost.network.reasons.upnpFailed",
  upnpUnavailable: "panes.selfHost.network.reasons.upnpUnavailable",
} as const satisfies Record<SelfHostReasonCode, TranslationKey>;

type Tone = "success" | "warning" | "danger" | "secondary";

/**
 * Findings that only confirm good news, or repeat what a family's badge
 * already says. The card lists the rest, and only when a peer cannot connect.
 */
const QUIET_REASONS: ReadonlySet<SelfHostReasonCode> = new Set([
  "firewallRuleMissing", // The card has its own row with the Allow button.
  "noPublicAddress",
  "portMapped",
  "probeReachable",
  "publicAddressOnDevice",
]);

/** What to do about the check, across both address families, each said once. */
export function actionableReasons(
  environment: Pick<SelfHostEnvironmentReport, "ipv4" | "ipv6">,
): SelfHostReasonCode[] {
  // The port-forward verdict already says the device is behind a router.
  const saidByVerdict = overallReachability(environment) === "needsPortForward" ? "behindNat" : null;
  const reasons = [...environment.ipv4.reasons, ...environment.ipv6.reasons];
  return [...new Set(reasons)].filter(
    (reason) => !QUIET_REASONS.has(reason) && reason !== saidByVerdict,
  );
}

/** A tone as a status dot, shared by the entry tiles and the network verdict. */
export const TONE_DOT_CLASS = {
  danger: "bg-danger",
  secondary: "bg-muted-foreground",
  success: "bg-success",
  warning: "bg-warning",
} as const satisfies Record<Tone, string>;

/** The badge color of a reachability verdict. */
export function reachabilityTone(reachability: SelfHostReachability): Tone {
  switch (reachability) {
    case "reachable":
      return "success";
    case "likelyReachable":
    case "needsPortForward":
      return "warning";
    case "unreachable":
      return "danger";
    case "noConnectivity":
    case "unknown":
      return "secondary";
  }
}

const REACHABILITY_RANK: readonly SelfHostReachability[] = [
  "reachable",
  "likelyReachable",
  "needsPortForward",
  "unreachable",
  "unknown",
  "noConnectivity",
];

/**
 * The check's overall verdict. A peer needs one address family only, so the
 * better of the two decides.
 */
export function overallReachability(
  environment: Pick<SelfHostEnvironmentReport, "ipv4" | "ipv6">,
): SelfHostReachability {
  const rank = (reachability: SelfHostReachability) => REACHABILITY_RANK.indexOf(reachability);
  const { ipv4, ipv6 } = environment;
  return rank(ipv4.reachability) <= rank(ipv6.reachability) ? ipv4.reachability : ipv6.reachability;
}

/** The status dot's state, which the shared dot styles color. */
export function statusTone(status: SelfHostRuntimeStatus): "on" | "busy" | "off" | "error" {
  switch (status) {
    case "running":
      return "on";
    case "starting":
    case "retrying":
      return "busy";
    case "stopped":
      return "off";
    case "failed":
      return "error";
  }
}
