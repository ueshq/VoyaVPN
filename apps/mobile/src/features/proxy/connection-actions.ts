import { voyaCommands } from "@voya/client/transport";

/** null belongs only to the explicit close-all action. */
export async function closeSingleConnection(id: string | null) {
  if (!id?.trim()) throw new Error("Connection identifier unavailable");
  await voyaCommands().proxyCloseConnection(id);
}
export async function closeAllConnections() {
  await voyaCommands().proxyCloseConnection(null);
}
