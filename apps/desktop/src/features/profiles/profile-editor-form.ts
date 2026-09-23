import type { ProfileKind } from "@voya/contracts";
import type { ProfileFormValues } from "@voya/features/profiles/profile-form-schema";
import { optionalNumber } from "@voya/features/profiles/profile-form-utils";

/**
 * The editor's draft. Numeric fields keep the string the user typed until
 * submit — the same shape the other dialogs use — and the schema is what
 * turns them into numbers.
 */
export type ProfileEditorForm = {
  configType: string;
  indexId: string;
  subscriptionId: string | null;
  displayLog: boolean;
  remarks: string;
  address: string;
  port: string;
  password: string;
  username: string;
  network: string;
  streamSecurity: string;
  sni: string;
  alpn: string;
  publicKey: string;
  shortId: string;
  cert: string;
  echConfigList: string;
  protocolOptions: {
    udpOverTcp: boolean;
    congestionControl: string;
    vmessCipher: string;
    flow: string;
    vlessEncryption: string;
    method: string;
    wireGuardPeerPublicKey: string;
    wireGuardPresharedKey: string;
    wireGuardInterfaceAddress: string;
    wireGuardAllowedIps: string;
    wireGuardReserved: string;
    wireGuardMtu: string;
    obfuscationPassword: string;
    portHops: string;
    insecureConcurrency: string;
    naiveQuic: boolean;
  };
  transportOptions: {
    header: string;
    host: string;
    path: string;
    grpcAuthority: string;
    grpcServiceName: string;
    grpcMode: string;
  };
};

/** Field path (dotted for nested) → already-translated message. */
export type ProfileFieldErrors = Record<string, string | undefined>;

function text(value: string | null | undefined) {
  return value ?? "";
}

function bool(value: boolean | null | undefined) {
  return value === true;
}

function draftNumber(value: number | string | null | undefined) {
  return value == null || value === "" ? "" : String(value);
}

export function toEditorForm(values: ProfileFormValues): ProfileEditorForm {
  // The schema's discriminated union always stores the full base shape; the
  // editor widens the numeric drafts to the strings the text fields bind.
  const source = values as unknown as ProfileEditorForm & {
    port: number | string;
    protocolOptions?: Partial<ProfileEditorForm["protocolOptions"]> | null;
    transportOptions?: Partial<ProfileEditorForm["transportOptions"]> | null;
  };
  const protocol = source.protocolOptions ?? {};
  const transport = source.transportOptions ?? {};
  return {
    configType: source.configType,
    indexId: source.indexId ?? "",
    subscriptionId: source.subscriptionId ?? null,
    displayLog: source.displayLog !== false,
    remarks: source.remarks ?? "",
    address: source.address ?? "",
    port: draftNumber(source.port),
    password: source.password ?? "",
    username: source.username ?? "",
    network: source.network ?? "tcp",
    streamSecurity: source.streamSecurity ?? "",
    sni: source.sni ?? "",
    alpn: source.alpn ?? "",
    publicKey: source.publicKey ?? "",
    shortId: source.shortId ?? "",
    cert: source.cert ?? "",
    echConfigList: source.echConfigList ?? "",
    protocolOptions: {
      udpOverTcp: bool(protocol.udpOverTcp),
      congestionControl: text(protocol.congestionControl),
      vmessCipher: text(protocol.vmessCipher),
      flow: text(protocol.flow),
      vlessEncryption: text(protocol.vlessEncryption),
      method: text(protocol.method),
      wireGuardPeerPublicKey: text(protocol.wireGuardPeerPublicKey),
      wireGuardPresharedKey: text(protocol.wireGuardPresharedKey),
      wireGuardInterfaceAddress: text(protocol.wireGuardInterfaceAddress),
      wireGuardAllowedIps: text(protocol.wireGuardAllowedIps),
      wireGuardReserved: text(protocol.wireGuardReserved),
      wireGuardMtu: draftNumber(protocol.wireGuardMtu),
      obfuscationPassword: text(protocol.obfuscationPassword),
      portHops: text(protocol.portHops),
      insecureConcurrency: draftNumber(protocol.insecureConcurrency),
      naiveQuic: bool(protocol.naiveQuic),
    },
    transportOptions: {
      header: text(transport.header),
      host: text(transport.host),
      path: text(transport.path),
      grpcAuthority: text(transport.grpcAuthority),
      grpcServiceName: text(transport.grpcServiceName),
      grpcMode: text(transport.grpcMode),
    },
  };
}

/** The schema's input: numeric drafts become numbers (or `NaN` / `null`). */
export function toSchemaValues(form: ProfileEditorForm): ProfileFormValues {
  return {
    ...form,
    configType: form.configType as ProfileKind,
    port: form.port.trim() === "" ? Number.NaN : Number(form.port),
    protocolOptions: {
      ...form.protocolOptions,
      wireGuardMtu: optionalNumber(form.protocolOptions.wireGuardMtu),
      insecureConcurrency: optionalNumber(form.protocolOptions.insecureConcurrency),
    },
  } as ProfileFormValues;
}
