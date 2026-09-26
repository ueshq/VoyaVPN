import type {
  CoreFlowReason,
  ImportLineCode,
  ImportLineIssue,
  LogCode,
  LogLineBody,
  NoticeCode,
  SpeedtestOutcome,
  ValidationCode,
  ValidationIssue,
  ValidationScope,
} from "@voya/contracts";
import type { TranslationFunction, TranslationKey } from "@voya/i18n/core";

/**
 * The one place a backend message code becomes text.
 *
 * The backend used to hand the frontend finished English sentences — notice
 * titles, validator messages, speedtest statuses, log lines — which the shipped
 * locales could not touch and `pnpm check:i18n` could not see, because
 * it only scans frontend source. Every one of those is a code now, and these
 * registries map it to the locale key that spells it out.
 *
 * Each map is `Record<Code, TranslationKey>`, so both halves are checked:
 * `tsc` fails when a Rust code has no entry (the generated union gained a
 * member) and when an entry names a key `en.json` does not define.
 *
 * The interpolation payload is the code object itself. `ValidationCode` and
 * friends are internally tagged, so `{ code: "unsupportedNetwork", network:
 * "kcp" }` already *is* the bag i18next reads `{{network}}` out of; there is no
 * second parameter list to keep in step.
 */
export const NOTICE_KEYS: Record<NoticeCode["code"], TranslationKey> = {
  activeProfileRestartFailed: "notices.activeProfileRestartFailed",
  policyGroupRefreshFailed: "notices.policyGroupRefreshFailed",
  policyGroupSavedRestartFailed: "notices.policyGroupSavedRestartFailed",
  policyGroupSelectionRuntimeUpdateFailed: "notices.policyGroupSelectionRuntimeUpdateFailed",
  connectionModeRefreshFailed: "notices.connectionModeRefreshFailed",
  connectionModeSavedRestartFailed: "notices.connectionModeSavedRestartFailed",
  coreStartedSystemProxyFailed: "notices.coreStartedSystemProxyFailed",
  activeSelectionRemoved: "notices.activeSelectionRemoved",
  coreStopped: "notices.coreStopped",
  dnsRefreshFailed: "notices.dnsRefreshFailed",
  dnsSavedRestartFailed: "notices.dnsSavedRestartFailed",
  nativeTunStopped: "notices.nativeTunStopped",
  nodeIpv6Restored: "notices.nodeIpv6Restored",
  nodeIpv6Unsupported: "notices.nodeIpv6Unsupported",
  profileRefreshFailed: "notices.profileRefreshFailed",
  proxyModeSavedRuntimeUpdateFailed:
    "notices.proxyModeSavedRuntimeUpdateFailed",
  proxyViewRefreshFailed: "notices.proxyViewRefreshFailed",
  routingDeletedRestartFailed: "notices.routingDeletedRestartFailed",
  routingRefreshFailed: "notices.routingRefreshFailed",
  routingRuleMovedRestartFailed: "notices.routingRuleMovedRestartFailed",
  routingRuleSavedRestartFailed: "notices.routingRuleSavedRestartFailed",
  routingRulesDeletedRestartFailed: "notices.routingRulesDeletedRestartFailed",
  routingRulesResetRestartFailed: "notices.routingRulesResetRestartFailed",
  routingSavedRestartFailed: "notices.routingSavedRestartFailed",
  routingSelectedRestartFailed: "notices.routingSelectedRestartFailed",
  selfHostAddressChanged: "notices.selfHostAddressChanged",
  selfHostGaveUp: "notices.selfHostGaveUp",
  selfHostRefreshFailed: "notices.selfHostRefreshFailed",
  settingsRefreshFailed: "notices.settingsRefreshFailed",
  settingsSavedRuntimeUpdateFailed: "notices.settingsSavedRuntimeUpdateFailed",
  settingsSavedSystemProxyUpdateFailed:
    "notices.settingsSavedSystemProxyUpdateFailed",
  subscriptionAutoUpdateFailed: "notices.subscriptionAutoUpdateFailed",
  subscriptionRefreshFailed: "notices.subscriptionRefreshFailed",
  systemProxyRestoreFailed: "notices.systemProxyRestoreFailed",
  systemProxyStatusRefreshFailed: "notices.systemProxyStatusRefreshFailed",
  trayActionFailed: "notices.trayActionFailed",
  trayRefreshFailed: "notices.trayRefreshFailed",
  tunSavedRestartFailed: "notices.tunSavedRestartFailed",
  tunStatusRefreshFailed: "notices.tunStatusRefreshFailed",
};

export const LOG_KEYS: Record<LogCode["code"], TranslationKey> = {
  connected: "logCodes.connected",
  connecting: "logCodes.connecting",
  coreExitGaveUp: "logCodes.coreExitGaveUp",
  coreExitRestarted: "logCodes.coreExitRestarted",
  coreExitRetryScheduled: "logCodes.coreExitRetryScheduled",
  coreOperationFailed: "logCodes.coreOperationFailed",
  disconnected: "logCodes.disconnected",
  disconnecting: "logCodes.disconnecting",
  nativeTunExited: "logCodes.nativeTunExited",
  postCommitFailed: "logCodes.postCommitFailed",
  previousCoreStillRunning: "logCodes.previousCoreStillRunning",
  restarted: "logCodes.restarted",
  restartedAfterChange: "logCodes.restartedAfterChange",
  restarting: "logCodes.restarting",
  restartingAfterChange: "logCodes.restartingAfterChange",
  runtimeStatusRefreshFailed: "logCodes.runtimeStatusRefreshFailed",
  selfHostGaveUp: "logCodes.selfHostGaveUp",
  selfHostPortMapped: "logCodes.selfHostPortMapped",
  selfHostPortMappingFailed: "logCodes.selfHostPortMappingFailed",
  selfHostRetryScheduled: "logCodes.selfHostRetryScheduled",
  selfHostStartFailed: "logCodes.selfHostStartFailed",
  selfHostStarted: "logCodes.selfHostStarted",
  selfHostStopped: "logCodes.selfHostStopped",
  speedtestCancellationRequested: "logCodes.speedtestCancellationRequested",
  subscriptionAutoUpdateFailed: "logCodes.subscriptionAutoUpdateFailed",
  subscriptionAutoUpdateFinished: "logCodes.subscriptionAutoUpdateFinished",
};

/** The operation a log sentence is about, interpolated into it as `reason`. */
export const CORE_FLOW_REASON_KEYS: Record<CoreFlowReason, TranslationKey> = {
  activeProfileChanged: "coreFlowReason.activeProfileChanged",
  policyGroupChanged: "coreFlowReason.policyGroupChanged",
  connect: "coreFlowReason.connect",
  connectionModeChanged: "coreFlowReason.connectionModeChanged",
  disconnect: "coreFlowReason.disconnect",
  dnsChanged: "coreFlowReason.dnsChanged",
  ipv6EgressChanged: "coreFlowReason.ipv6EgressChanged",
  restart: "coreFlowReason.restart",
  routingChanged: "coreFlowReason.routingChanged",
  settingsSaved: "coreFlowReason.settingsSaved",
  tunChanged: "coreFlowReason.tunChanged",
};

export const VALIDATION_KEYS: Record<ValidationCode["code"], TranslationKey> = {
  subscriptionReadOnly: "validation.subscriptionReadOnly",
  dnsAddressEmpty: "validation.dnsAddressEmpty",
  dnsAddressPort: "validation.dnsAddressPort",
  dnsExpectedIps: "validation.expectedIps",
  dnsHostsLine: "validation.dnsHostsLine",
  fragmentFallbackDelayOutOfRange: "validation.fragmentFallbackDelayOutOfRange",
  hysteriaHopIntervalTooShort: "validation.hysteriaHopIntervalTooShort",
  inboundCredentialsIncomplete: "validation.inboundCredentialsIncomplete",
  inboundPortOutOfRange: "validation.inboundPortOutOfRange",
  inboundRequired: "validation.inboundRequired",
  invalidAddress: "validation.invalidAddress",
  invalidFlow: "validation.invalidFlow",
  invalidPassword: "validation.invalidPassword",
  invalidPort: "validation.invalidPort",
  invalidRealityPublicKey: "validation.invalidRealityPublicKey",
  invalidShadowsocksMethod: "validation.invalidShadowsocksMethod",
  negativeHysteriaBandwidth: "validation.negativeHysteriaBandwidth",
  policyGroupIntervalOutOfRange: "validation.policyGroupIntervalOutOfRange",
  policyGroupTestUrlInvalid: "validation.policyGroupTestUrlInvalid",
  policyGroupToleranceOutOfRange: "validation.policyGroupToleranceOutOfRange",
  policyGroupWithoutMembers: "validation.policyGroupWithoutMembers",
  policyGroupWithoutValidMembers: "validation.policyGroupWithoutValidMembers",
  routingRuleOutboundNotFound: "validation.routingRuleOutboundNotFound",
  routingRuleWithoutOutbound: "validation.routingRuleWithoutOutbound",
  selfHostNoProtocol: "validation.selfHostNoProtocol",
  selfHostPortOutOfRange: "validation.selfHostPortOutOfRange",
  selfHostPortsCollide: "validation.selfHostPortsCollide",
  textControlCharacters: "validation.textControlCharacters",
  textRequired: "validation.textRequired",
  textTooLong: "validation.textTooLong",
  tooManyItems: "validation.tooManyItems",
  tunMtuOutOfRange: "validation.tunMtuOutOfRange",
  unsupportedProtocolNetwork: "validation.unsupportedProtocolNetwork",
  unsupportedShadowsocksNetwork: "validation.unsupportedShadowsocksNetwork",
  // Rendered from its own English `message`, never through a key.
  untranslated: "validation.invalid",
};

export const VALIDATION_SCOPE_KEYS: Record<
  ValidationScope["kind"],
  TranslationKey
> = {
  policyGroupMember: "validation.scope.policyGroupMember",
  routingRuleOutbound: "validation.scope.routingRuleOutbound",
};

export const SPEEDTEST_OUTCOME_KEYS: Record<SpeedtestOutcome, TranslationKey> =
  {
    cancelled: "speedtest.outcome.cancelled",
    completed: "speedtest.outcome.completed",
    coreUnavailable: "speedtest.outcome.coreUnavailable",
    failed: "speedtest.outcome.failed",
    invalidProfile: "speedtest.outcome.invalidProfile",
    noAvailablePort: "speedtest.outcome.noAvailablePort",
    proxyConnectFailed: "speedtest.outcome.proxyConnectFailed",
    proxyConnectionClosed: "speedtest.outcome.proxyConnectionClosed",
    proxyConnectionRefused: "speedtest.outcome.proxyConnectionRefused",
    reconnectRequired: "speedtest.outcome.reconnectRequired",
    skipped: "speedtest.outcome.skipped",
    testing: "speedtest.outcome.testing",
    timedOut: "speedtest.outcome.timedOut",
    unknown: "speedtest.outcome.unknown",
    waiting: "speedtest.outcome.waiting",
  };

export const IMPORT_LINE_KEYS: Record<ImportLineCode["code"], TranslationKey> = {
  invalidPort: "panes.profiles.import.line.invalidPort",
  missingField: "panes.profiles.import.line.missingField",
  parseFailed: "panes.profiles.import.line.parseFailed",
  subscriptionSourceAdded: "panes.profiles.import.line.subscriptionSourceAdded",
  unsupportedProtocol: "panes.profiles.import.line.unsupportedProtocol",
  unsupportedTransport: "panes.profiles.import.line.unsupportedTransport",
};

export function noticeText(t: TranslationFunction, code: NoticeCode) {
  return t(NOTICE_KEYS[code.code], code);
}

/**
 * One log line as the panel shows it.
 *
 * Core output and `tracing` diagnostics are passed through; only the app's own
 * sentences are translated, with their untranslated `detail` after them.
 */
export function logLineText(t: TranslationFunction, body: LogLineBody) {
  if (body.source !== "app") {
    return body.line;
  }

  const reason =
    "reason" in body.code
      ? t(CORE_FLOW_REASON_KEYS[body.code.reason])
      : undefined;
  const text = t(LOG_KEYS[body.code.code], { ...body.code, reason });

  return body.detail ? `${text}: ${body.detail}` : text;
}

/**
 * One validation finding, with the breadcrumb the validator walked to reach it.
 *
 * `untranslated` is the escape hatch for managers that have no code yet, and
 * shows their English diagnostic rather than a locale key nobody wrote.
 */
export function validationText(t: TranslationFunction, issue: ValidationIssue) {
  const message =
    issue.code.code === "untranslated"
      ? issue.code.message
      : t(VALIDATION_KEYS[issue.code.code], {
          ...issue.code,
        });
  if (issue.scope.length === 0) {
    return message;
  }

  const breadcrumb = issue.scope
    .map((scope) => t(VALIDATION_SCOPE_KEYS[scope.kind], scope))
    .join(" / ");

  return `${breadcrumb}: ${message}`;
}

export function validationFieldErrors(
  t: TranslationFunction,
  issues: readonly ValidationIssue[],
): Record<string, string> {
  return Object.fromEntries(
    issues.map((issue) => [issue.field, validationText(t, issue)]),
  );
}

export function speedtestOutcomeText(
  t: TranslationFunction,
  outcome: SpeedtestOutcome,
) {
  return t(SPEEDTEST_OUTCOME_KEYS[outcome]);
}

/** One reported line of an import, with its 1-based line number. */
export function importLineText(t: TranslationFunction, issue: ImportLineIssue) {
  return t(IMPORT_LINE_KEYS[issue.code.code], {
    ...issue.code,
    line: issue.line,
  });
}
