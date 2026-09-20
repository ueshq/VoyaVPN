import { IpcCommandError } from "@voya/client/errors";
import type { AppError, VoyaCommands, VoyaEventName, VoyaEventPayload } from "@voya/contracts";
import type { TurboModule } from "react-native";
import { VOYA_COMMAND_WIRE } from "@voya/contracts/commands";

import type { VoyaTransport } from "./transport";

/**
 * The native module `crates/voya-mobile-ffi` is fronted by.
 *
 * Declared here rather than imported from a generated file: uniffi generates
 * Swift and Kotlin, not TypeScript, and this is the whole surface either one
 * exposes — one envelope in, one envelope out, plus the event stream. See
 * ADR 0012 for why the boundary is an envelope and not a modelled API.
 */
export type VoyaCommandInvoker = {
  /**
   * Runs one backend command. `argsJson` is the same named-argument object
   * Tauri receives, and the answer is the command's return value as JSON.
   *
   * A rejection carries a serialized `AppError` in its message, because uniffi
   * errors travel as a type rather than as arbitrary data.
   */
  invoke: (command: string, argsJson: string) => Promise<string>;
};

export type VoyaNativeModule = TurboModule &
  VoyaCommandInvoker & {
    /**
     * The two methods React Native requires of any module it emits events
     * from. They are the module's own bookkeeping; nothing here calls them.
     */
    addListener: (eventName: string) => void;
    removeListeners: (count: number) => void;
  };

/** The three channels, as React Native delivers them. */
export type VoyaNativeEvents = {
  addListener: (
    channel: string,
    listener: (payload: { channel: string; payloadJson: string }) => void,
  ) => { remove: () => void };
};

/** The wire name each channel is published under, shared with the host. */
const CHANNEL_NAMES = {
  appEvent: "app-event",
  invalidateEvent: "invalidate-event",
  transientStreamEvent: "transient-stream-event",
} as const satisfies Record<VoyaEventName, string>;

/** The one event the native module emits; the channel is in the payload. */
const NATIVE_EVENT = "VoyaBackendEvent";

/**
 * The `VoyaCommands` surface over the native module.
 *
 * Built from `VOYA_COMMAND_WIRE` rather than written out: the table already
 * says what every command is called and which arguments it takes, in the order
 * `VoyaCommands` passes them, so seventy hand-written wrappers could only ever
 * drift from it. The generated table is checked against Rust by
 * `pnpm check:bindings`, which makes this loop correct by construction.
 */
function nativeCommands(native: VoyaCommandInvoker): VoyaCommands {
  const entries = Object.entries(VOYA_COMMAND_WIRE).map(([method, { name, params }]) => [
    method,
    async (...args: unknown[]) => {
      // Tauri takes a named object, so the positional call is rebuilt into
      // one. An argument the caller left off stays absent rather than becoming
      // `undefined`: `serde` reads a missing field and a null differently.
      const named: Record<string, unknown> = {};
      params.forEach((param, index) => {
        if (index < args.length) named[param] = args[index];
      });

      return parseAnswer(name, await invoke(native, name, named));
    },
  ]);

  // Sound by construction: the table is keyed by `keyof VoyaCommands` and each
  // entry answers with that command's own return value, parsed from its JSON.
  return Object.fromEntries(entries) as VoyaCommands;
}

async function invoke(
  native: VoyaCommandInvoker,
  name: string,
  named: Record<string, unknown>,
): Promise<string> {
  try {
    return await native.invoke(name, JSON.stringify(named));
  } catch (error) {
    throw new IpcCommandError(appErrorFrom(error, name));
  }
}

/**
 * The `AppError` behind a native rejection.
 *
 * The host rejects with a serialized `AppError` so a screen branches on the
 * typed `kind` on both platforms. Anything else — a module that is not there,
 * a bridge that died mid-call — is not the backend refusing, so it is reported
 * as what it is rather than dressed up as a backend error with a made-up kind.
 */
function appErrorFrom(error: unknown, command: string): AppError {
  const message = error instanceof Error ? error.message : String(error);
  const parsed = tryParse(message);
  if (isAppError(parsed)) return parsed;

  return {
    kind: { type: "internal" },
    message: `${command} could not reach the backend: ${message}`,
    subsystem: "app",
  };
}

function parseAnswer(command: string, json: string): unknown {
  // A command with no return value answers `null`, which parses fine; only a
  // malformed answer lands here, and that is the transport's fault, not the
  // command's.
  const parsed = tryParse(json);
  if (parsed === undefined) {
    throw new IpcCommandError({
      kind: { type: "internal" },
      message: `the answer to ${command} was not valid JSON`,
      subsystem: "app",
    });
  }

  return parsed;
}

function tryParse(text: string): unknown {
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return undefined;
  }
}

function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<AppError>;

  return (
    typeof candidate.message === "string" &&
    typeof candidate.subsystem === "string" &&
    typeof candidate.kind === "object" &&
    candidate.kind !== null &&
    typeof (candidate.kind as { type?: unknown }).type === "string"
  );
}

/**
 * The transport a device build registers.
 *
 * The same shape `createTransport` builds over the shared mock, so nothing
 * downstream of `setVoyaCommands` can tell which one answered — which is what
 * makes the mock build a real rehearsal rather than a separate app.
 */
export function createNativeTransport(
  native: VoyaCommandInvoker,
  events: VoyaNativeEvents,
): VoyaTransport {
  return {
    commands: nativeCommands(native),
    on: <Name extends VoyaEventName>(
      name: Name,
      listener: (payload: VoyaEventPayload<Name>) => void,
    ) => {
      const channel = CHANNEL_NAMES[name];
      const subscription = events.addListener(NATIVE_EVENT, (event) => {
        if (event.channel !== channel) return;
        const payload = tryParse(event.payloadJson);
        if (payload === undefined) {
          // A malformed payload is a host bug; dropping the event keeps the
          // app running with a stale cache rather than crashing the bridge.
          console.warn(`ignoring a malformed ${channel} payload`);
          return;
        }
        listener(payload as VoyaEventPayload<Name>);
      });

      return () => subscription.remove();
    },
  };
}
