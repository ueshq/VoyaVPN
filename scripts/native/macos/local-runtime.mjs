import { capture, commandFailure, validateTiming } from "../../lib/common.mjs";
import { appBundleIdentifier as defaultProviderId } from "./tunnel-layout.mjs";

const packetTunnelExecutable = "VoyaPacketTunnel";
const defaultReplacementTarget = "/Applications/VoyaVPN.app";
const defaultGuiExecutables = ["voyavpn", "VoyaVPN"];

/** Every VoyaVPN executable whose presence blocks a destructive local mutation. */
export const voyaRuntimeExecutables = [...defaultGuiExecutables, packetTunnelExecutable];
const sleeper = new Int32Array(new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT));

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * `pgrep -x` presence probe, exported so machine-global tools (the
 * NetworkExtension doctor) can refuse to mutate PlugInKit under a live app
 * instead of hand-rolling a fourth copy of this check.
 */
export function defaultIsProcessRunning(executable) {
  const program = "/usr/bin/pgrep";
  const args = ["-x", executable];
  const result = capture(program, args);
  if (result.error) {
    throw result.error;
  }
  if (result.status === 0) {
    return true;
  }
  if (result.status === 1) {
    return false;
  }
  throw commandFailure(program, args, result);
}

function defaultListConnections() {
  const program = "/usr/sbin/scutil";
  const args = ["--nc", "list"];
  const result = capture(program, args);
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw commandFailure(program, args, result);
  }
  return parseScutilNetworkConnections(result.stdout ?? "");
}

function defaultStopConnection(id) {
  const program = "/usr/sbin/scutil";
  const args = ["--nc", "stop", id];
  const result = capture(program, args);
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw commandFailure(program, args, result);
  }
}

function defaultWait(milliseconds) {
  Atomics.wait(sleeper, 0, 0, milliseconds);
}

function isDisconnected(connection) {
  return connection.state.trim().toLowerCase() === "disconnected";
}

/**
 * Parse only scutil network connections owned by the exact VPN provider id.
 *
 * @returns {Array<{id: string, state: string}>}
 */
export function parseScutilNetworkConnections(output, providerId = defaultProviderId) {
  const providerMarker = new RegExp(`\\[VPN:${escapeRegExp(providerId)}\\](?:\\s|$)`);
  const connectionPattern = /^\s*\*?\s*\(([^)]+)\)\s+([0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12})\b/;
  const connections = [];

  for (const line of String(output ?? "").split(/\r?\n/)) {
    if (!providerMarker.test(line)) {
      continue;
    }
    const match = line.match(connectionPattern);
    if (!match) {
      continue;
    }
    connections.push({
      id: match[2],
      state: match[1].trim(),
    });
  }

  return connections;
}

/**
 * Gracefully stop VoyaVPN's NetworkExtension before replacing the local app.
 * This function never kills processes or removes VPN profiles.
 */
export function prepareVoyaForLocalBuild({
  guiExecutables = defaultGuiExecutables,
  replacementTarget = defaultReplacementTarget,
  isProcessRunning = defaultIsProcessRunning,
  listConnections = defaultListConnections,
  stopConnection = defaultStopConnection,
  wait = defaultWait,
  logger = console,
  timeoutMs = 20_000,
  pollIntervalMs = 250,
} = {}) {
  validateTiming(timeoutMs, pollIntervalMs);

  const runningGuiExecutables = [...new Set(guiExecutables)]
    .filter((executable) => executable && isProcessRunning(executable));
  if (runningGuiExecutables.length > 0) {
    throw new Error(
      `VoyaVPN is still running (${runningGuiExecutables.join(", ")}). Quit the app before replacing ${replacementTarget}.`,
    );
  }

  const initialConnections = listConnections();
  const activeConnections = initialConnections.filter((connection) => !isDisconnected(connection));
  const stoppedConnectionIds = [];

  for (const connection of activeConnections) {
    logger?.log?.(
      `Stopping VoyaVPN connection ${connection.id} (${connection.state}) before replacing ${replacementTarget}.`,
    );
    stopConnection(connection.id);
    stoppedConnectionIds.push(connection.id);
  }

  let elapsedMs = 0;
  while (true) {
    const connections = listConnections();
    const connectionsStopped = connections.every(isDisconnected);
    const providerStopped = !isProcessRunning(packetTunnelExecutable);
    if (connectionsStopped && providerStopped) {
      return {
        elapsedMs,
        stoppedConnectionIds,
      };
    }

    if (elapsedMs >= timeoutMs) {
      const activeIds = connections
        .filter((connection) => !isDisconnected(connection))
        .map((connection) => connection.id);
      const detail = activeIds.length > 0
        ? ` Active connection(s): ${activeIds.join(", ")}.`
        : " The VoyaPacketTunnel process is still running.";
      throw new Error(
        `Timed out after ${timeoutMs}ms waiting for VoyaVPN to stop before replacing ${replacementTarget}.${detail}`,
      );
    }

    const delayMs = Math.min(pollIntervalMs, timeoutMs - elapsedMs);
    wait(delayMs);
    elapsedMs += delayMs;
  }
}
