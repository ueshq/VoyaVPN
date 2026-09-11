import { commands } from "@/ipc/bindings";
import type { AppError, ConnectionMode, MoveAction } from "@/ipc/bindings";

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

export const loadUiPreferences = wrapCommand(commands.loadUiPreferences);

export const loadAppSettings = wrapCommand(commands.loadAppSettings);

export const saveAppSettings = wrapCommand(commands.saveAppSettings);

export const generateQrCode = wrapCommand(commands.generateQrCode);

export const scanScreenQr = wrapCommand(commands.scanScreenQr);

export const fetchCertificate = wrapCommand(commands.fetchCertificate);

export const calculateCertificateSha256 = wrapCommand(commands.calculateCertificateSha256);

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

export const listProfiles = wrapCommand(
  (subscriptionId: string | null = null, filter: string | null = null) =>
    commands.listProfiles(subscriptionId, filter),
);

export const saveProfile = wrapCommand(commands.saveProfile);

export const deleteProfiles = wrapCommand(commands.deleteProfiles);

export const exportProfileShareLinks = wrapCommand(commands.exportProfileShareLinks);

export const exportProfileShareLinksBase64 = wrapCommand(commands.exportProfileShareLinksBase64);

export const exportProfileVoyaBundle = wrapCommand(commands.exportProfileVoyaBundle);

export const setActiveProfile = wrapCommand(commands.setActiveProfile);

export const moveProfile = wrapCommand(
  (
    subscriptionId: string | null,
    indexId: string,
    action: MoveAction,
    position: number | null = null,
  ) => commands.moveProfile(subscriptionId, indexId, action, position),
);

export const listSubscriptions = wrapCommand(commands.listSubscriptions);

export const saveSubscription = wrapCommand(commands.saveSubscription);

export const deleteSubscriptions = wrapCommand(commands.deleteSubscriptions);

export const importProfilesFromText = wrapCommand(
  (text: string, subscriptionId: string | null = null) =>
    commands.importProfilesFromText(text, subscriptionId),
);

export const updateSubscriptions = wrapCommand(
  (
    subscriptionId: string | null = null,
    preferProxy: boolean = true,
    proxyUrl: string | null = null,
  ) => commands.updateSubscriptions(subscriptionId, preferProxy, proxyUrl),
);

export const listSubscriptionMetadata = wrapCommand(commands.listSubscriptionMetadata);

export const connectionModeStatus = wrapCommand(commands.connectionModeStatus);

export const setConnectionMode = wrapCommand(
  (mode: ConnectionMode, pacEnabled: boolean | null = null) =>
    commands.setConnectionMode(mode, pacEnabled),
);

export const listProcessCandidates = wrapCommand(commands.listProcessCandidates);

export const listRoutings = wrapCommand(commands.listRoutings);

export const saveRouting = wrapCommand(commands.saveRouting);

export const deleteRoutings = wrapCommand(commands.deleteRoutings);

export const setActiveRouting = wrapCommand(commands.setActiveRouting);

export const saveRoutingRule = wrapCommand(commands.saveRoutingRule);

export const deleteRoutingRules = wrapCommand(commands.deleteRoutingRules);

export const moveRoutingRule = wrapCommand(
  (
    routingId: string,
    ruleId: string,
    action: MoveAction,
    position: number | null = null,
  ) => commands.moveRoutingRule(routingId, ruleId, action, position),
);

export const proxyListConnections = wrapCommand(commands.proxyListConnections);

export const proxyCloseConnection = wrapCommand(
  (connectionId: string | null = null) => commands.proxyCloseConnection(connectionId),
);

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

export const recheckSystemProxy = wrapCommand(commands.recheckSystemProxy);

export async function openNetworkSettings(): Promise<void> {
  unwrapCommandResult(await commands.openNetworkSettings());
}

export const listNodeGroups = wrapCommand(commands.listNodeGroups);
export const saveNodeGroup = wrapCommand(commands.saveNodeGroup);
export const deleteNodeGroup = wrapCommand(commands.deleteNodeGroup);
export const moveNodeGroup = wrapCommand(commands.moveNodeGroup);
export const assignNodeGroups = wrapCommand(commands.assignNodeGroups);

export const copyProfiles = wrapCommand(commands.copyProfiles);

export const updateNodeGroup = wrapCommand(commands.updateNodeGroup);

export const getSettingsApplyStatus = wrapCommand(commands.getSettingsApplyStatus);

export const applyPendingSettings = wrapCommand(commands.applyPendingSettings);
