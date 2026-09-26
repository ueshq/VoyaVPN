import { TurboModuleRegistry, type TurboModule } from "react-native";

type DeviceActions = TurboModule & {
  /** Android only: iOS asks while saving the tunnel preferences instead. */
  requestVpnAuthorization(): Promise<boolean>;
  scanQr(cancelLabel: string): Promise<string[] | null>;
  pickQr(): Promise<string[] | null>;
  shareDiagnostics(text: string): Promise<void>;
  appVersion(): Promise<string>;
};

/**
 * What the app asks of the device itself: VPN consent, the QR camera and photo
 * picker, the share sheet and the app version.
 *
 * Both hosts register `VoyaDeviceActions` the way they register `VoyaNative`,
 * as a bridge module, and it is looked up the same way `ipc/transport.ts`
 * looks that one up: the new architecture's interop layer serves bridge
 * modules through `TurboModuleRegistry` too.
 */
export function deviceActions(): DeviceActions {
  const module = TurboModuleRegistry.get<DeviceActions>("VoyaDeviceActions");
  if (!module) throw new Error("Device actions are unavailable in this build");
  return module;
}
