import { NativeModules, type TurboModule } from "react-native";

type DeviceActions = TurboModule & {
  requestVpnAuthorization(): Promise<boolean>;
  scanQr(cancelLabel: string): Promise<string[] | null>;
  pickQr(): Promise<string[] | null>;
  shareDiagnostics(text: string): Promise<void>;
  appVersion(): Promise<string>;
};
function device(): DeviceActions {
  const module = NativeModules.VoyaDeviceActions as DeviceActions | undefined;
  if (!module) throw new Error("Device actions are unavailable in this build");
  return module;
}
export function requestVpnAuthorization() { return device().requestVpnAuthorization(); }
export function scanQr(cancelLabel: string) { return device().scanQr(cancelLabel); }
export function pickQr() { return device().pickQr(); }
export function shareDiagnostics(text: string) { return device().shareDiagnostics(text); }
export function appVersion() { return device().appVersion(); }
