import { describe, expect, it, vi } from "vite-plus/test";

import { parseScutilNetworkConnections, prepareVoyaForLocalBuild } from "./local-runtime.mjs";

const voyaId = "9E69EDC4-0CFD-4A90-BDFA-7AE633F1C16C";
const secondVoyaId = "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE";
const v2boxId = "41E5C1B6-13AF-49E8-8AE6-D84DB819ECB2";
const replacementTarget = "/Applications/VoyaVPN.app";

function connection(state, id = voyaId) {
  return { id, state };
}

function options(overrides = {}) {
  return {
    guiExecutables: ["voyavpn"],
    replacementTarget,
    isProcessRunning: vi.fn(() => false),
    listConnections: vi.fn(() => []),
    stopConnection: vi.fn(),
    wait: vi.fn(),
    logger: { log: vi.fn() },
    timeoutMs: 100,
    pollIntervalMs: 25,
    ...overrides,
  };
}

describe("parseScutilNetworkConnections", () => {
  it("parses only the exact Voya provider and leaves V2BOX unaffected", () => {
    const output = `Available network connection services in the current set (*=enabled):
* (Connected)      ${v2boxId} VPN (hossin.asaadi.V2Box) "V2BOX" [VPN:hossin.asaadi.V2Box]
* (Connected)      AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE VPN (app.voyavpn.desktop.other) "Other" [VPN:app.voyavpn.desktop.other]
* (Disconnected)   ${voyaId} VPN (app.voyavpn.desktop) "VoyaVPN" [VPN:app.voyavpn.desktop]`;

    expect(parseScutilNetworkConnections(output)).toEqual([{ id: voyaId, state: "Disconnected" }]);
  });
});

describe("prepareVoyaForLocalBuild", () => {
  it("blocks immediately when the GUI is running and never stops a connection", () => {
    const listConnections = vi.fn(() => [connection("Connected")]);
    const stopConnection = vi.fn();
    const runtimeOptions = options({
      isProcessRunning: vi.fn((executable) => executable === "voyavpn"),
      listConnections,
      stopConnection,
    });

    expect(() => prepareVoyaForLocalBuild(runtimeOptions)).toThrow(/VoyaVPN is still running.*voyavpn/);
    expect(listConnections).not.toHaveBeenCalled();
    expect(stopConnection).not.toHaveBeenCalled();
  });

  it("does not stop an already disconnected Voya service", () => {
    const stopConnection = vi.fn();
    const runtimeOptions = options({
      listConnections: vi.fn(() => [connection("Disconnected")]),
      stopConnection,
    });

    const result = prepareVoyaForLocalBuild(runtimeOptions);

    expect(stopConnection).not.toHaveBeenCalled();
    expect(result).toEqual({ elapsedMs: 0, stoppedConnectionIds: [] });
  });

  it("gracefully stops a connected Voya service by UUID and waits for shutdown", () => {
    const listConnections = vi
      .fn()
      .mockReturnValueOnce([connection("Connected")])
      .mockReturnValueOnce([connection("Disconnecting")])
      .mockReturnValue([connection("Disconnected")]);
    const providerStates = [true, false];
    const isProcessRunning = vi.fn((executable) => {
      if (executable !== "VoyaPacketTunnel") {
        return false;
      }
      return providerStates.shift() ?? false;
    });
    const stopConnection = vi.fn();
    const wait = vi.fn();
    const runtimeOptions = options({
      isProcessRunning,
      listConnections,
      stopConnection,
      wait,
    });

    const result = prepareVoyaForLocalBuild(runtimeOptions);

    expect(stopConnection).toHaveBeenCalledOnce();
    expect(stopConnection).toHaveBeenCalledWith(voyaId);
    expect(wait).toHaveBeenCalledOnce();
    expect(wait).toHaveBeenCalledWith(25);
    expect(result).toEqual({ elapsedMs: 25, stoppedConnectionIds: [voyaId] });
  });

  it("stops only active services when multiple Voya profiles exist", () => {
    const listConnections = vi
      .fn()
      .mockReturnValueOnce([connection("Disconnected"), connection("Connecting", secondVoyaId)])
      .mockReturnValue([connection("Disconnected"), connection("Disconnected", secondVoyaId)]);
    const stopConnection = vi.fn();
    const runtimeOptions = options({ listConnections, stopConnection });

    const result = prepareVoyaForLocalBuild(runtimeOptions);

    expect(stopConnection).toHaveBeenCalledOnce();
    expect(stopConnection).toHaveBeenCalledWith(secondVoyaId);
    expect(result.stoppedConnectionIds).toEqual([secondVoyaId]);
  });

  it("fails closed when the provider lingers without a matching connection", () => {
    const stopConnection = vi.fn();
    const runtimeOptions = options({
      isProcessRunning: vi.fn((executable) => executable === "VoyaPacketTunnel"),
      stopConnection,
      timeoutMs: 50,
    });

    expect(() => prepareVoyaForLocalBuild(runtimeOptions)).toThrow(/The VoyaPacketTunnel process is still running/);
    expect(stopConnection).not.toHaveBeenCalled();
  });

  it("times out without killing the provider or deleting its VPN profile", () => {
    const stopConnection = vi.fn();
    const wait = vi.fn();
    const runtimeOptions = options({
      isProcessRunning: vi.fn((executable) => executable === "VoyaPacketTunnel"),
      listConnections: vi.fn(() => [connection("Connected")]),
      stopConnection,
      wait,
    });

    expect(() => prepareVoyaForLocalBuild(runtimeOptions)).toThrow(
      /Timed out after 100ms.*9E69EDC4-0CFD-4A90-BDFA-7AE633F1C16C/,
    );
    expect(stopConnection).toHaveBeenCalledOnce();
    expect(stopConnection).toHaveBeenCalledWith(voyaId);
    expect(wait).toHaveBeenCalledTimes(4);
    expect(wait).toHaveBeenLastCalledWith(25);
  });
});
