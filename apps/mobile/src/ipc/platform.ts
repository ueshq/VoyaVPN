import Clipboard from "@react-native-clipboard/clipboard";
import { Platform } from "react-native";
import { requestVpnAuthorization } from "~/native/device-actions";
import { setClipboard, setElevationHandler } from "@voya/client/platform";
import { setVoyaCommands } from "@voya/client/transport";

import { createTransport, type VoyaTransport } from "./transport";

/**
 * The backend registration, split out of `platform-boot` so the event bridge
 * can reach the same instance.
 *
 * `setVoyaCommands` puts the command half behind `@voya/client`, which is all
 * a feature ever sees; the event half has no global home, because subscribing
 * is a component's lifecycle, not a module's.
 */
let transport: VoyaTransport | null = null;

export function registerMobileBackend() {
  transport = createTransport();
  setVoyaCommands(transport.commands);
  // iOS requests authorization while saving NETunnelProvider preferences.
  // Android must launch VpnService.prepare from the foreground Activity.
  setElevationHandler(() => Platform.OS === "android" ? requestVpnAuthorization() : Promise.resolve(false));

  // Both halves are native here. The desktop reads through the backend because
  // a WebView read needs a user gesture; React Native has no such restriction,
  // so `use-node-import` reaches the same clipboard either way.
  setClipboard({
    readText: () => Clipboard.getString(),
    writeText: async (text) => Clipboard.setString(text),
  });
}

export function voyaTransport(): VoyaTransport {
  if (!transport) {
    throw new Error("No transport registered — call registerMobileBackend() during app startup.");
  }

  return transport;
}
