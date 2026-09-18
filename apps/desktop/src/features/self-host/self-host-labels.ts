import type {
  SelfHostAddressKind,
  SelfHostFirewallStatus,
  SelfHostNatKind,
  SelfHostPortMappingStatus,
  SelfHostProblem,
  SelfHostReachability,
  SelfHostReasonCode,
  SelfHostRuntimeStatus,
  SelfHostSelfTestResult,
} from "@/ipc/bindings";
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

export const NAT_KEYS = {
  carrierGrade: "panes.selfHost.network.nat.carrierGrade",
  double: "panes.selfHost.network.nat.double",
  nat: "panes.selfHost.network.nat.nat",
  noConnectivity: "panes.selfHost.network.nat.noConnectivity",
  none: "panes.selfHost.network.nat.none",
  unknown: "panes.selfHost.network.nat.unknown",
} as const satisfies Record<SelfHostNatKind, TranslationKey>;

export const REACHABILITY_KEYS = {
  likelyReachable: "panes.selfHost.network.reachability.likelyReachable",
  needsPortForward: "panes.selfHost.network.reachability.needsPortForward",
  noConnectivity: "panes.selfHost.network.reachability.noConnectivity",
  reachable: "panes.selfHost.network.reachability.reachable",
  unknown: "panes.selfHost.network.reachability.unknown",
  unreachable: "panes.selfHost.network.reachability.unreachable",
} as const satisfies Record<SelfHostReachability, TranslationKey>;

export const REASON_KEYS = {
  behindNat: "panes.selfHost.network.reasons.behindNat",
  carrierGradeNat: "panes.selfHost.network.reasons.carrierGradeNat",
  doubleNat: "panes.selfHost.network.reasons.doubleNat",
  firewallRuleMissing: "panes.selfHost.network.reasons.firewallRuleMissing",
  ipv6FirewallUnknown: "panes.selfHost.network.reasons.ipv6FirewallUnknown",
  noPublicAddress: "panes.selfHost.network.reasons.noPublicAddress",
  nodeNotRunning: "panes.selfHost.network.reasons.nodeNotRunning",
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

export const PORT_MAPPING_KEYS = {
  disabled: "panes.selfHost.network.upnp.disabled",
  failed: "panes.selfHost.network.upnp.failed",
  mapped: "panes.selfHost.network.upnp.mapped",
  noGateway: "panes.selfHost.network.upnp.noGateway",
} as const satisfies Record<SelfHostPortMappingStatus, TranslationKey>;

export const FIREWALL_KEYS = {
  notManaged: "panes.selfHost.network.firewall.notManaged",
  ruleMissing: "panes.selfHost.network.firewall.ruleMissing",
  rulePresent: "panes.selfHost.network.firewall.rulePresent",
  unknown: "panes.selfHost.network.firewall.unknown",
} as const satisfies Record<SelfHostFirewallStatus, TranslationKey>;

export const SELF_TEST_KEYS = {
  failed: "panes.selfHost.network.selfTest.failed",
  passed: "panes.selfHost.network.selfTest.passed",
  skipped: "panes.selfHost.network.selfTest.skipped",
} as const satisfies Record<SelfHostSelfTestResult, TranslationKey>;

type Tone = "success" | "warning" | "danger" | "secondary";

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

/** The badge color of a self-test verdict. */
export function selfTestTone(result: SelfHostSelfTestResult): Tone {
  switch (result) {
    case "passed":
      return "success";
    case "failed":
      return "danger";
    case "skipped":
      return "secondary";
  }
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
