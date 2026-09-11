import { commands } from "@/ipc/bindings";
import type {
  AppError,
  NodeGroup,
  NodeGroupAssignment,
  NodeGroupsSnapshot,
  AppUpdaterStatus,
  CertificateFetchRequest,
  CertificateFetchResult,
  ProxyConnectionsSnapshot,
  ProxyMonitorStatus,
  DnsSettings,
  ExportProfilesResult,
  ImportProfilesResult,
  MoveAction,
  Profile,
  ProfileListEntry,
  ProfileListing,
  QrCodeImage,
  QrScanResult,
  Routing_Deserialize,
  Routing_Serialize,
  TrafficMode,
  TrafficModeResponse,
  RoutingRule,
  RuntimeStatusResponse,
  CoreSeedInstallResult,
  CoreType,
  ResourceUpdateFile,
  SpeedtestRequest,
  SpeedtestRunResult,
  SpeedtestStatus,
  Subscription,
  SubscriptionMetadata,
  SubscriptionUpdateResult,
  ConnectionMode,
  ConnectionModeStatus,
  ProcessCandidate,
  SystemProxyStatusResponse,
  TunProviderDiagnostics,
  TunStatus,
  AppearanceSettings,
  AppSettingsV1,
  WindowChromeConfig,
} from "@/ipc/bindings";

type CommandResult<T> =
  { status: "ok"; data: T } | { status: "error"; error: AppError };

export class IpcCommandError extends Error {
  readonly appError: AppError;

  constructor(appError: AppError) {
    super(formatAppError(appError));
    this.appError = appError;
    this.name = "IpcCommandError";
  }
}

export async function loadUiPreferences(): Promise<AppearanceSettings> {
  return unwrapCommandResult(await commands.loadUiPreferences());
}

export async function loadAppSettings(): Promise<AppSettingsV1> {
  return unwrapCommandResult(await commands.loadAppSettings());
}

export async function saveAppSettings(
  settings: AppSettingsV1,
): Promise<AppSettingsV1> {
  return unwrapCommandResult(await commands.saveAppSettings(settings));
}

export async function generateQrCode(content: string): Promise<QrCodeImage> {
  return unwrapCommandResult(await commands.generateQrCode(content));
}

export async function scanScreenQr(): Promise<QrScanResult> {
  return unwrapCommandResult(await commands.scanScreenQr());
}

export async function fetchCertificate(
  request: CertificateFetchRequest,
): Promise<CertificateFetchResult> {
  return unwrapCommandResult(await commands.fetchCertificate(request));
}

export async function calculateCertificateSha256(
  pem: string,
): Promise<string[]> {
  return unwrapCommandResult(await commands.calculateCertificateSha256(pem));
}

export async function connectActiveProfile(): Promise<RuntimeStatusResponse> {
  return unwrapCommandResult(await commands.connectActiveProfile());
}

export async function disconnectCore(): Promise<RuntimeStatusResponse> {
  return unwrapCommandResult(await commands.disconnectCore());
}

export async function restartCore(): Promise<RuntimeStatusResponse> {
  return unwrapCommandResult(await commands.restartCore());
}

export async function runtimeStatus(): Promise<RuntimeStatusResponse> {
  return unwrapCommandResult(await commands.runtimeStatus());
}

export async function systemProxyStatus(): Promise<SystemProxyStatusResponse> {
  return unwrapCommandResult(await commands.systemProxyStatus());
}

export async function tunStatus(): Promise<TunStatus> {
  return unwrapCommandResult(await commands.tunStatus());
}

export async function tunProviderDiagnostics(): Promise<TunProviderDiagnostics> {
  return unwrapCommandResult(await commands.tunProviderDiagnostics());
}

export async function tunRequestElevation(): Promise<TunStatus> {
  return unwrapCommandResult(await commands.tunRequestElevation());
}

export async function loadDnsSettings(): Promise<DnsSettings> {
  return unwrapCommandResult(await commands.loadDnsSettings());
}

export async function saveDnsSettings(
  settings: DnsSettings,
): Promise<DnsSettings> {
  return unwrapCommandResult(await commands.saveDnsSettings(settings));
}

export async function listProfiles(
  subscriptionId: string | null = null,
  filter: string | null = null,
): Promise<ProfileListing> {
  return unwrapCommandResult(
    await commands.listProfiles(subscriptionId, filter),
  );
}

export async function saveProfile(profile: Profile): Promise<ProfileListEntry> {
  return unwrapCommandResult(await commands.saveProfile(profile));
}

export async function deleteProfiles(indexIds: string[]): Promise<number> {
  return unwrapCommandResult(await commands.deleteProfiles(indexIds));
}

export async function exportProfileShareLinks(
  indexIds: string[],
): Promise<ExportProfilesResult> {
  return unwrapCommandResult(await commands.exportProfileShareLinks(indexIds));
}

export async function exportProfileShareLinksBase64(
  indexIds: string[],
): Promise<ExportProfilesResult> {
  return unwrapCommandResult(
    await commands.exportProfileShareLinksBase64(indexIds),
  );
}

export async function exportProfileVoyaBundle(
  indexIds: string[],
): Promise<ExportProfilesResult> {
  return unwrapCommandResult(await commands.exportProfileVoyaBundle(indexIds));
}

export async function setActiveProfile(
  indexId: string,
): Promise<ProfileListEntry> {
  return unwrapCommandResult(await commands.setActiveProfile(indexId));
}

export async function moveProfile(
  subscriptionId: string | null,
  indexId: string,
  action: MoveAction,
  position: number | null = null,
): Promise<ProfileListEntry[]> {
  return unwrapCommandResult(
    await commands.moveProfile(subscriptionId, indexId, action, position),
  );
}

export async function listSubscriptions(): Promise<Subscription[]> {
  return unwrapCommandResult(await commands.listSubscriptions());
}

export async function saveSubscription(
  item: Subscription,
): Promise<Subscription> {
  return unwrapCommandResult(await commands.saveSubscription(item));
}

export async function deleteSubscriptions(ids: string[]): Promise<number> {
  return unwrapCommandResult(await commands.deleteSubscriptions(ids));
}

export async function importProfilesFromText(
  text: string,
  subscriptionId: string | null = null,
): Promise<ImportProfilesResult> {
  return unwrapCommandResult(
    await commands.importProfilesFromText(text, subscriptionId),
  );
}

export async function updateSubscriptions(
  subscriptionId: string | null = null,
  preferProxy = true,
  proxyUrl: string | null = null,
): Promise<SubscriptionUpdateResult> {
  return unwrapCommandResult(
    await commands.updateSubscriptions(subscriptionId, preferProxy, proxyUrl),
  );
}

export async function listSubscriptionMetadata(): Promise<
  SubscriptionMetadata[]
> {
  return unwrapCommandResult(await commands.listSubscriptionMetadata());
}

export async function connectionModeStatus(): Promise<ConnectionModeStatus> {
  return unwrapCommandResult(await commands.connectionModeStatus());
}

export async function setConnectionMode(
  mode: ConnectionMode,
  pacEnabled: boolean | null = null,
): Promise<ConnectionModeStatus> {
  return unwrapCommandResult(
    await commands.setConnectionMode(mode, pacEnabled),
  );
}

export async function listProcessCandidates(): Promise<ProcessCandidate[]> {
  return unwrapCommandResult(await commands.listProcessCandidates());
}

export async function listRoutings(): Promise<Routing_Serialize[]> {
  return unwrapCommandResult(await commands.listRoutings());
}

export async function saveRouting(
  item: Routing_Deserialize,
): Promise<Routing_Serialize> {
  return unwrapCommandResult(await commands.saveRouting(item));
}

export async function deleteRoutings(ids: string[]): Promise<number> {
  return unwrapCommandResult(await commands.deleteRoutings(ids));
}

export async function setActiveRouting(id: string): Promise<Routing_Serialize> {
  return unwrapCommandResult(await commands.setActiveRouting(id));
}

export async function saveRoutingRule(
  routingId: string,
  rule: RoutingRule,
): Promise<Routing_Serialize> {
  return unwrapCommandResult(await commands.saveRoutingRule(routingId, rule));
}

export async function deleteRoutingRules(
  routingId: string,
  ruleIds: string[],
): Promise<Routing_Serialize> {
  return unwrapCommandResult(
    await commands.deleteRoutingRules(routingId, ruleIds),
  );
}

export async function moveRoutingRule(
  routingId: string,
  ruleId: string,
  action: MoveAction,
  position: number | null = null,
): Promise<Routing_Serialize> {
  return unwrapCommandResult(
    await commands.moveRoutingRule(routingId, ruleId, action, position),
  );
}

export async function proxyListConnections(): Promise<ProxyConnectionsSnapshot> {
  return unwrapCommandResult(await commands.proxyListConnections());
}

export async function proxyCloseConnection(
  connectionId: string | null = null,
): Promise<ProxyConnectionsSnapshot> {
  return unwrapCommandResult(await commands.proxyCloseConnection(connectionId));
}

export async function proxySetTrafficMode(
  mode: TrafficMode,
): Promise<TrafficModeResponse> {
  return unwrapCommandResult(await commands.proxySetTrafficMode(mode));
}

export async function proxyStartMonitor(): Promise<ProxyMonitorStatus> {
  return unwrapCommandResult(await commands.proxyStartMonitor());
}

export async function proxyStopMonitor(): Promise<ProxyMonitorStatus> {
  return unwrapCommandResult(await commands.proxyStopMonitor());
}

export async function runSpeedtest(
  request: SpeedtestRequest,
): Promise<SpeedtestRunResult> {
  return unwrapCommandResult(await commands.runSpeedtest(request));
}

export async function cancelSpeedtest(): Promise<SpeedtestStatus> {
  return unwrapCommandResult(await commands.cancelSpeedtest());
}

export async function speedtestStatus(): Promise<SpeedtestStatus> {
  return unwrapCommandResult(await commands.speedtestStatus());
}

export async function appUpdateStatus(): Promise<AppUpdaterStatus> {
  return unwrapCommandResult(await commands.appUpdateStatus());
}

export async function updateGeoAssets(): Promise<ResourceUpdateFile[]> {
  return unwrapCommandResult(await commands.updateGeoAssets());
}

export async function updateSrsAssets(): Promise<ResourceUpdateFile[]> {
  return unwrapCommandResult(await commands.updateSrsAssets());
}

export async function installCoreSeed(
  coreType: CoreType,
): Promise<CoreSeedInstallResult> {
  return unwrapCommandResult(await commands.installCoreSeed(coreType));
}

export async function getWindowChromeConfig(): Promise<WindowChromeConfig> {
  return unwrapCommandResult(await commands.getWindowChromeConfig());
}

export async function setWindowAcrylic(dark: boolean): Promise<null> {
  return unwrapCommandResult(await commands.setWindowAcrylic(dark));
}

function unwrapCommandResult<T>(result: CommandResult<T>): T {
  if (result.status === "error") {
    throw new IpcCommandError(result.error);
  }

  return result.data;
}

/**
 * Every `AppError` carries its own diagnostic text, so formatting is a field
 * read rather than a switch.
 *
 * This used to be a 23-arm switch in which 21 arms returned `error.message`
 * unchanged and the other two reached one level deeper. Anything a caller wants
 * to *decide* now lives in `error.appError.kind`, which is typed.
 */
function formatAppError(error: AppError): string {
  return error.message;
}

export async function recheckSystemProxy(): Promise<SystemProxyStatusResponse> {
  return unwrapCommandResult(await commands.recheckSystemProxy());
}

export async function openNetworkSettings(): Promise<void> {
  unwrapCommandResult(await commands.openNetworkSettings());
}

export async function listNodeGroups(): Promise<NodeGroupsSnapshot> {
  return unwrapCommandResult(await commands.listNodeGroups());
}
export async function saveNodeGroup(
  id: string | null,
  name: string,
): Promise<NodeGroup> {
  return unwrapCommandResult(await commands.saveNodeGroup(id, name));
}
export async function deleteNodeGroup(id: string): Promise<null> {
  return unwrapCommandResult(await commands.deleteNodeGroup(id));
}
export async function moveNodeGroup(
  id: string,
  action: MoveAction,
): Promise<null> {
  return unwrapCommandResult(await commands.moveNodeGroup(id, action));
}
export async function assignNodeGroups(
  assignments: NodeGroupAssignment[],
): Promise<null> {
  return unwrapCommandResult(await commands.assignNodeGroups(assignments));
}

export async function copyProfiles(
  indexIds: string[],
): Promise<ProfileListEntry[]> {
  return unwrapCommandResult(await commands.copyProfiles(indexIds));
}

export async function updateNodeGroup(
  id: string,
  name: string,
  assignments: NodeGroupAssignment[],
): Promise<NodeGroup> {
  return unwrapCommandResult(
    await commands.updateNodeGroup(id, name, assignments),
  );
}

export async function getSettingsApplyStatus() {
  return unwrapCommandResult(await commands.getSettingsApplyStatus());
}

export async function applyPendingSettings() {
  return unwrapCommandResult(await commands.applyPendingSettings());
}
