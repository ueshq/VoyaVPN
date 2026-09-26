import { NativeEventEmitter, TurboModuleRegistry } from "react-native";
import type { VoyaCommands, VoyaEventName, VoyaEventPayload } from "@voya/contracts";

import {
  createNativeTransport,
  type VoyaNativeEvents,
  type VoyaNativeModule,
} from "./native-transport";

/**
 * What this app needs of a backend: the command surface and the three channels.
 *
 * The same shape the shared mock backend has, so a test registers the mock
 * exactly where a device build registers the native module.
 */
export type VoyaTransport = {
  commands: VoyaCommands;
  on: <Name extends VoyaEventName>(
    name: Name,
    listener: (payload: VoyaEventPayload<Name>) => void,
  ) => () => void;
};

/** The name the native module registers itself under, on both platforms. */
const NATIVE_MODULE_NAME = "VoyaNative";

/**
 * The native backend. Both native projects register the module
 * unconditionally, so its absence means the Rust host was not built
 * (`pnpm native:mobile:rust:ios` / `:android`); tests register the shared
 * mock instead and never reach this.
 */
export function createTransport(): VoyaTransport {
  const native = TurboModuleRegistry.getEnforcing<VoyaNativeModule>(NATIVE_MODULE_NAME);

  return createNativeTransport(native, nativeEvents(native));
}

/**
 * The host publishes all three channels as one native event with the channel
 * in its payload, so there is one listener rather than three registrations to
 * keep in step.
 */
function nativeEvents(native: VoyaNativeModule): VoyaNativeEvents {
  const emitter = new NativeEventEmitter(native);

  return {
    addListener: (name, listener) =>
      emitter.addListener(name, (payload) =>
        listener(payload as { channel: string; payloadJson: string }),
      ),
  };
}
