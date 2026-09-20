import { IpcCommandError } from "@voya/client/errors";
import { VOYA_COMMAND_WIRE } from "@voya/contracts/commands";

import {
  createNativeTransport,
  type VoyaCommandInvoker,
  type VoyaNativeEvents,
} from "./native-transport";

type Call = { command: string; argsJson: string };

function nativeModule(answer: (call: Call) => Promise<string>) {
  const calls: Call[] = [];

  return {
    calls,
    module: {
      invoke: (command: string, argsJson: string) => {
        const call = { argsJson, command };
        calls.push(call);

        return answer(call);
      },
    } satisfies VoyaCommandInvoker,
  };
}

function nativeEvents() {
  const listeners = new Set<(payload: { channel: string; payloadJson: string }) => void>();

  return {
    emit(channel: string, payloadJson: string) {
      for (const listener of listeners) listener({ channel, payloadJson });
    },
    events: {
      addListener: (_name, listener) => {
        listeners.add(listener);

        return {
          remove: () => {
            listeners.delete(listener);
          },
        };
      },
    } satisfies VoyaNativeEvents,
    get listenerCount() {
      return listeners.size;
    },
  };
}

describe("native transport", () => {
  it("answers the whole command surface, so nothing reaches a screen as a TypeError", () => {
    const { module } = nativeModule(() => Promise.resolve("null"));
    const { commands } = createNativeTransport(module, nativeEvents().events);

    for (const method of Object.keys(VOYA_COMMAND_WIRE)) {
      expect(typeof commands[method as keyof typeof commands]).toBe("function");
    }
  });

  it("sends the backend's own name and a named argument object", async () => {
    const { calls, module } = nativeModule(() => Promise.resolve('{"imported":1}'));
    const { commands } = createNativeTransport(module, nativeEvents().events);

    await commands.importProfilesFromText("vless://token@example.test:443", null);

    expect(calls).toEqual([
      {
        command: "import_profiles_from_text",
        argsJson: JSON.stringify({
          text: "vless://token@example.test:443",
          subscriptionId: null,
        }),
      },
    ]);
  });

  it("leaves an omitted argument out rather than sending undefined", async () => {
    const { calls, module } = nativeModule(() => Promise.resolve("null"));
    const { commands } = createNativeTransport(module, nativeEvents().events);

    // `serde` reads a missing field and an explicit null differently, so an
    // argument the caller never passed must not become one.
    await (commands.updateSubscriptions as (id: string) => Promise<unknown>)("subscription-0");

    expect(calls[0]?.argsJson).toBe(JSON.stringify({ subscriptionId: "subscription-0" }));
  });

  it("parses the answer", async () => {
    const { module } = nativeModule(() => Promise.resolve('{"running":true}'));
    const { commands } = createNativeTransport(module, nativeEvents().events);

    await expect(commands.speedtestStatus()).resolves.toEqual({ running: true });
  });

  it("rejects with the backend's typed error, not its text", async () => {
    const appError = {
      kind: { type: "validation", issues: [] },
      message: "DNS rejected",
      subsystem: "dns",
    };
    const { module } = nativeModule(() => Promise.reject(new Error(JSON.stringify(appError))));
    const { commands } = createNativeTransport(module, nativeEvents().events);

    await expect(commands.loadDnsSettings()).rejects.toMatchObject({
      appError,
      name: "IpcCommandError",
    });
  });

  it("reports a bridge that is not there as a transport failure", async () => {
    const { module } = nativeModule(() => Promise.reject(new Error("VoyaNative is null")));
    const { commands } = createNativeTransport(module, nativeEvents().events);

    // Not the backend refusing, so it must not arrive wearing a made-up kind.
    const error = await commands.runtimeStatus().catch((reason: unknown) => reason);
    expect(error).toBeInstanceOf(IpcCommandError);
    expect((error as IpcCommandError).appError).toEqual({
      kind: { type: "internal" },
      message: "runtime_status could not reach the backend: VoyaNative is null",
      subsystem: "app",
    });
  });

  it("refuses an answer that is not JSON at all", async () => {
    const { module } = nativeModule(() => Promise.resolve("<html>proxy error</html>"));
    const { commands } = createNativeTransport(module, nativeEvents().events);

    await expect(commands.runtimeStatus()).rejects.toThrow(/not valid JSON/);
  });

  it("routes each channel to its own subscriber and unsubscribes", () => {
    const { module } = nativeModule(() => Promise.resolve("null"));
    const native = nativeEvents();
    const transport = createNativeTransport(module, native.events);
    const invalidations: unknown[] = [];
    const transients: unknown[] = [];

    const stop = transport.on("invalidateEvent", (payload) => invalidations.push(payload));
    transport.on("transientStreamEvent", (payload) => transients.push(payload));

    native.emit("invalidate-event", '{"keys":[]}');
    native.emit("transient-stream-event", '{"kind":"logLines","payload":[]}');

    expect(invalidations).toEqual([{ keys: [] }]);
    expect(transients).toEqual([{ kind: "logLines", payload: [] }]);

    stop();
    native.emit("invalidate-event", '{"keys":[]}');
    expect(invalidations).toHaveLength(1);
    expect(native.listenerCount).toBe(1);
  });

  it("drops a malformed payload instead of taking the bridge down with it", () => {
    const { module } = nativeModule(() => Promise.resolve("null"));
    const native = nativeEvents();
    const transport = createNativeTransport(module, native.events);
    const received: unknown[] = [];
    const warn = jest.spyOn(console, "warn").mockImplementation(() => {});

    transport.on("appEvent", (payload) => received.push(payload));
    native.emit("app-event", "{not json");
    native.emit("app-event", '{"kind":"closeRequested"}');

    expect(received).toEqual([{ kind: "closeRequested" }]);
    expect(warn).toHaveBeenCalledWith("ignoring a malformed app-event payload");
    warn.mockRestore();
  });
});
