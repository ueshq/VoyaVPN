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
}

export function voyaTransport(): VoyaTransport {
  if (!transport) {
    throw new Error("No transport registered — call registerMobileBackend() during app startup.");
  }

  return transport;
}
