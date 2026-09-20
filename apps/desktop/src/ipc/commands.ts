import { commands } from "@/ipc/bindings";
import type { AppError, AppErrorKind } from "@/ipc/bindings";

type CommandResult<T> =
  { status: "ok"; data: T } | { status: "error"; error: AppError };

export class IpcCommandError extends Error {
  readonly appError: AppError;

  constructor(appError: AppError) {
    super(appError.message);
    this.appError = appError;
    this.name = "IpcCommandError";
  }
}

/** A rejected command's backend error when it is of the given kind, otherwise `null`. */
export function appErrorOfKind<Type extends AppErrorKind["type"]>(
  error: unknown,
  type: Type,
): (AppError & { kind: Extract<AppErrorKind, { type: Type }> }) | null {
  return error instanceof IpcCommandError && error.appError.kind.type === type
    ? (error.appError as AppError & { kind: Extract<AppErrorKind, { type: Type }> })
    : null;
}

export const loadUiPreferences = wrapCommand(commands.loadUiPreferences);

export const loadAppSettings = wrapCommand(commands.loadAppSettings);

export const saveAppSettings = wrapCommand(commands.saveAppSettings);

export const getSettingsApplyStatus = wrapCommand(commands.getSettingsApplyStatus);

export const applyPendingSettings = wrapCommand(commands.applyPendingSettings);

export const generateQrCode = wrapCommand(commands.generateQrCode);

export const scanScreenQr = wrapCommand(commands.scanScreenQr);

/** Decodes the QR codes in a picture already turned into grey pixels. */
export const decodeQrImage = wrapCommand(commands.decodeQrImage);

export const readClipboardText = wrapCommand(commands.readClipboardText);

/** Asks where to save log text; `false` when the save dialog is cancelled. */
export const exportLogs = wrapCommand(commands.exportLogs);

/** Whether the Logs panel is showing, and so wants log lines delivered. */
export const setLogStreaming = wrapCommand(commands.setLogStreaming);

export const connectActiveProfile = wrapCommand(commands.connectActiveProfile);

export const disconnectCore = wrapCommand(commands.disconnectCore);

export const restartCore = wrapCommand(commands.restartCore);

export const runtimeStatus = wrapCommand(commands.runtimeStatus);

export const systemProxyStatus = wrapCommand(commands.systemProxyStatus);

export const tunStatus = wrapCommand(commands.tunStatus);

export const tunProviderDiagnostics = wrapCommand(commands.tunProviderDiagnostics);

export const tunRequestElevation = wrapCommand(commands.tunRequestElevation);

export const loadDnsSettings = wrapCommand(commands.loadDnsSettings);

export const saveDnsSettings = wrapCommand(commands.saveDnsSettings);

/** Every node as the node table shows it; `getProfile` has one in full. */
export const listProfileSummaries = wrapCommand(commands.listProfileSummaries);

/** One node in full: credentials, transport, TLS and traffic. */
export const getProfile = wrapCommand(commands.getProfile);

export const saveProfile = wrapCommand(commands.saveProfile);

export const deleteProfiles = wrapCommand(commands.deleteProfiles);

export const exportProfileShareLinks = wrapCommand(commands.exportProfileShareLinks);

export const setActiveProfile = wrapCommand(commands.setActiveProfile);

export const listPolicyGroups = wrapCommand(commands.listPolicyGroups);

export const savePolicyGroup = wrapCommand(commands.savePolicyGroup);

export const deletePolicyGroups = wrapCommand(commands.deletePolicyGroups);

export const setActivePolicyGroup = wrapCommand(commands.setActivePolicyGroup);

export const selectPolicyGroupMember = wrapCommand(commands.selectPolicyGroupMember);

export const policyGroupRuntime = wrapCommand(commands.policyGroupRuntime);

export const testPolicyGroupDelay = wrapCommand(commands.testPolicyGroupDelay);

export const moveProfile = wrapCommand(commands.moveProfile);

export const listSubscriptions = wrapCommand(commands.listSubscriptions);

export const saveSubscription = wrapCommand(commands.saveSubscription);

export const deleteSubscriptions = wrapCommand(commands.deleteSubscriptions);

export const importProfilesFromText = wrapCommand(commands.importProfilesFromText);

export const updateSubscriptions = wrapCommand(commands.updateSubscriptions);

export const listSubscriptionMetadata = wrapCommand(commands.listSubscriptionMetadata);

export const connectionModeStatus = wrapCommand(commands.connectionModeStatus);

export const setConnectionMode = wrapCommand(commands.setConnectionMode);

export const checkConnectionIp = wrapCommand(commands.checkConnectionIp);

export const resolveCloseRequest = wrapCommand(commands.resolveCloseRequest);

export const listProcessCandidates = wrapCommand(commands.listProcessCandidates);

export const listRoutings = wrapCommand(commands.listRoutings);

export const saveRoutingRule = wrapCommand(commands.saveRoutingRule);

export const deleteRoutingRules = wrapCommand(commands.deleteRoutingRules);

export const resetRoutingRules = wrapCommand(commands.resetRoutingRules);

export const moveRoutingRule = wrapCommand(commands.moveRoutingRule);

export const proxyListConnections = wrapCommand(commands.proxyListConnections);

export const proxyCloseConnection = wrapCommand(commands.proxyCloseConnection);

export const proxySetTrafficMode = wrapCommand(commands.proxySetTrafficMode);

export const proxyStartMonitor = wrapCommand(commands.proxyStartMonitor);

export const proxyStopMonitor = wrapCommand(commands.proxyStopMonitor);

export const runSpeedtest = wrapCommand(commands.runSpeedtest);

export const cancelSpeedtest = wrapCommand(commands.cancelSpeedtest);

export const speedtestStatus = wrapCommand(commands.speedtestStatus);

export const appUpdateStatus = wrapCommand(commands.appUpdateStatus);

export const updateGeoAssets = wrapCommand(commands.updateGeoAssets);

export const updateSrsAssets = wrapCommand(commands.updateSrsAssets);

export const installCoreSeed = wrapCommand(commands.installCoreSeed);

export const getSelfHostState = wrapCommand(commands.getSelfHostState);

export const saveSelfHostConfig = wrapCommand(commands.saveSelfHostConfig);

export const setSelfHostEnabled = wrapCommand(commands.setSelfHostEnabled);

export const rotateSelfHostCredentials = wrapCommand(commands.rotateSelfHostCredentials);

export const getSelfHostStats = wrapCommand(commands.getSelfHostStats);

export const runSelfHostEnvironmentCheck = wrapCommand(commands.runSelfHostEnvironmentCheck);

/** Adds the Windows Firewall rule for the node; blocks on a UAC prompt. */
export const applySelfHostFirewallRule = wrapCommand(commands.applySelfHostFirewallRule);

export const getWindowChromeConfig = wrapCommand(commands.getWindowChromeConfig);

export const setWindowAcrylic = wrapCommand(commands.setWindowAcrylic);

function wrapCommand<Args extends unknown[], T>(
  command: (...args: Args) => Promise<CommandResult<T>>,
) {
  return async (...args: Args): Promise<T> => unwrapCommandResult(await command(...args));
}

function unwrapCommandResult<T>(result: CommandResult<T>): T {
  if (result.status === "error") {
    throw new IpcCommandError(result.error);
  }

  return result.data;
}
