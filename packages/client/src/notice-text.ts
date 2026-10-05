import type { NoticeCode } from "@voya/contracts";
import type { TranslationFunction, TranslationKey } from "@voya/i18n/core";

/**
 * Notice codes and the locale keys that spell them out — `messages.ts` holds
 * the other registries and says how they all work. This one is apart because
 * the event router raises notices from the first moment the shell is up, and a
 * module the startup path imports is loaded whole: beside the notices it would
 * bring every log, validation and speedtest key with it.
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
  settingsSavedSystemProxyUpdateFailed:
    "notices.settingsSavedSystemProxyUpdateFailed",
  subscriptionAutoUpdateFailed: "notices.subscriptionAutoUpdateFailed",
  subscriptionRefreshFailed: "notices.subscriptionRefreshFailed",
  systemProxyRestoreFailed: "notices.systemProxyRestoreFailed",
  systemProxyStatusRefreshFailed: "notices.systemProxyStatusRefreshFailed",
  trayActionFailed: "notices.trayActionFailed",
  trayRefreshFailed: "notices.trayRefreshFailed",
  tunStatusRefreshFailed: "notices.tunStatusRefreshFailed",
};

export function noticeText(t: TranslationFunction, code: NoticeCode) {
  return t(NOTICE_KEYS[code.code], code);
}
