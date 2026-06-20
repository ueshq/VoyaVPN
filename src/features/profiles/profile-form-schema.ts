import { z } from "zod";

import type { ProfileItem_Deserialize, ProfileItem_Serialize } from "@/ipc/bindings";

import { CONFIG_TYPES, CORE_TYPES, type ProfileProtocol } from "./profile-constants";

const optionalText = z.string().optional();
const optionalNullableText = z.string().nullable().optional();
const optionalNullableBool = z.boolean().nullable().optional();
const optionalNullableNumber = z.number().int().nullable().optional();

const protocolExtraSchema = z
  .object({
    Uot: optionalNullableBool,
    CongestionControl: optionalNullableText,
    AlterId: optionalNullableText,
    VmessSecurity: optionalNullableText,
    Flow: optionalNullableText,
    VlessEncryption: optionalNullableText,
    SsMethod: optionalNullableText,
    WgPublicKey: optionalNullableText,
    WgPresharedKey: optionalNullableText,
    WgInterfaceAddress: optionalNullableText,
    WgAllowedIps: optionalNullableText,
    WgReserved: optionalNullableText,
    WgMtu: optionalNullableNumber,
    SalamanderPass: optionalNullableText,
    UpMbps: optionalNullableNumber,
    DownMbps: optionalNullableNumber,
    Ports: optionalNullableText,
    HopInterval: optionalNullableText,
    InsecureConcurrency: optionalNullableNumber,
    NaiveQuic: optionalNullableBool,
    GroupType: optionalNullableText,
    ChildItems: optionalNullableText,
    SubChildItems: optionalNullableText,
    Filter: optionalNullableText,
    MultipleLoad: optionalNullableNumber,
  })
  .default({});

const transportExtraSchema = z
  .object({
    RawHeaderType: optionalNullableText,
    Host: optionalNullableText,
    Path: optionalNullableText,
    XhttpMode: optionalNullableText,
    XhttpExtra: optionalNullableText,
    GrpcAuthority: optionalNullableText,
    GrpcServiceName: optionalNullableText,
    GrpcMode: optionalNullableText,
    KcpHeaderType: optionalNullableText,
    KcpSeed: optionalNullableText,
    KcpMtu: optionalNullableNumber,
  })
  .default({});

const commonProfileSchema = z.object({
  IndexId: optionalText,
  CoreType: z.number().int().nullable().optional(),
  ConfigVersion: z.number().int().default(4),
  Subid: optionalText,
  IsSub: z.boolean().default(false),
  PreSocksPort: optionalNullableNumber,
  DisplayLog: z.boolean().default(true),
  Remarks: z.string().trim().min(1, "Remarks are required"),
  Address: z.string().trim().min(1, "Address is required"),
  Port: z.number().int().min(0).max(65535),
  Password: optionalText,
  Username: optionalText,
  Network: optionalText,
  StreamSecurity: optionalText,
  AllowInsecure: optionalText,
  Sni: optionalText,
  Alpn: optionalText,
  Fingerprint: optionalText,
  PublicKey: optionalText,
  ShortId: optionalText,
  SpiderX: optionalText,
  Mldsa65Verify: optionalText,
  MuxEnabled: optionalNullableBool,
  Cert: optionalText,
  CertSha: optionalText,
  EchConfigList: optionalText,
  Finalmask: optionalText,
  ProtocolExtra: protocolExtraSchema,
  TransportExtra: transportExtraSchema,
});

const serverProfileSchema = commonProfileSchema.extend({
  Password: z.string().trim().min(1, "Password or ID is required"),
});

const authProfileSchema = commonProfileSchema.extend({
  Password: optionalText,
  Username: optionalText,
});

export const profileFormSchema = z.discriminatedUnion("ConfigType", [
  serverProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.VMess) }),
  commonProfileSchema.extend({
    Address: z.string().trim().min(1, "Config path or JSON source is required"),
    ConfigType: z.literal(CONFIG_TYPES.Custom),
    CoreType: z.number().int().default(CORE_TYPES.Xray),
    Port: z.number().int().min(0).max(65535).default(0),
  }),
  serverProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.Shadowsocks) }),
  authProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.SOCKS) }),
  serverProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.VLESS) }),
  serverProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.Trojan) }),
  serverProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.Hysteria2) }),
  serverProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.TUIC) }),
  serverProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.WireGuard) }),
  authProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.HTTP) }),
  serverProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.Anytls) }),
  authProfileSchema.extend({ ConfigType: z.literal(CONFIG_TYPES.Naive) }),
  commonProfileSchema.extend({
    Address: z.string().default("group"),
    ConfigType: z.literal(CONFIG_TYPES.PolicyGroup),
    Port: z.number().int().default(0),
  }),
  commonProfileSchema.extend({
    Address: z.string().default("chain"),
    ConfigType: z.literal(CONFIG_TYPES.ProxyChain),
    Port: z.number().int().default(0),
  }),
]);

export type ProfileFormValues = z.input<typeof profileFormSchema>;
export type ParsedProfileFormValues = z.output<typeof profileFormSchema>;

export function createDefaultProfile(configType: ProfileProtocol = CONFIG_TYPES.VMess): ProfileFormValues {
  return createBaseProfile(configType) as ProfileFormValues;
}

export function normalizeProfileForForm(profile: ProfileItem_Deserialize | ProfileItem_Serialize): ProfileFormValues {
  const configType = Number(profile.ConfigType ?? CONFIG_TYPES.VMess) as ProfileProtocol;
  const candidate = {
    ...createBaseProfile(configType),
    ...profile,
    ConfigType: configType,
    CoreType: profile.CoreType ?? null,
    Port: Number(profile.Port ?? defaultPort(configType)),
    IsSub: profile.IsSub ?? false,
    DisplayLog: profile.DisplayLog ?? true,
    ProtocolExtra: { ...(profile.ProtocolExtra ?? {}) },
    TransportExtra: { ...(profile.TransportExtra ?? {}) },
  };

  return candidate as ProfileFormValues;
}

export function prepareProfileForSave(values: ProfileFormValues | ParsedProfileFormValues): ProfileItem_Deserialize {
  const parsed = profileFormSchema.parse(values);

  return parsedProfileToIpcPayload(parsed);
}

export function prepareGroupDraftForPreview(
  values: ProfileFormValues | ParsedProfileFormValues,
): ProfileItem_Deserialize {
  const configType = Number((values as { ConfigType?: number }).ConfigType ?? CONFIG_TYPES.PolicyGroup) as ProfileProtocol;
  const draft = {
    ...createBaseProfile(configType),
    ...(values as Record<string, unknown>),
    Address: (values as { Address?: string }).Address || defaultAddress(configType),
    ConfigType: configType,
    Remarks: (values as { Remarks?: string }).Remarks?.trim() || "Draft group",
    Port: Number((values as { Port?: number }).Port ?? 0),
    ProtocolExtra: {
      ...((values as { ProtocolExtra?: Record<string, unknown> }).ProtocolExtra ?? {}),
    },
    TransportExtra: {
      ...((values as { TransportExtra?: Record<string, unknown> }).TransportExtra ?? {}),
    },
  };

  return parsedProfileToIpcPayload(profileFormSchema.parse(draft));
}

function parsedProfileToIpcPayload(parsed: ParsedProfileFormValues): ProfileItem_Deserialize {
  return scrubEmptyStrings({
    ...parsed,
    CoreType: parsed.CoreType ?? null,
    ConfigVersion: 4,
    Port: Number(parsed.Port ?? 0),
    ProtocolExtra: scrubEmptyStrings(parsed.ProtocolExtra ?? {}),
    TransportExtra: scrubEmptyStrings(parsed.TransportExtra ?? {}),
  }) as ProfileItem_Deserialize;
}

function createBaseProfile(configType: ProfileProtocol): ProfileItem_Deserialize {
  return {
    ConfigType: configType,
    CoreType: null,
    ConfigVersion: 4,
    Subid: "",
    IsSub: false,
    DisplayLog: true,
    Remarks: "",
    Address: defaultAddress(configType),
    Port: defaultPort(configType),
    Password: "",
    Username: "",
    Network: "tcp",
    StreamSecurity: "",
    AllowInsecure: "false",
    Sni: "",
    Alpn: "",
    Fingerprint: "",
    PublicKey: "",
    ShortId: "",
    SpiderX: "",
    Mldsa65Verify: "",
    MuxEnabled: false,
    Cert: "",
    CertSha: "",
    EchConfigList: "",
    Finalmask: "",
    ProtocolExtra: {},
    TransportExtra: {},
  };
}

function defaultAddress(configType: ProfileProtocol) {
  if (configType === CONFIG_TYPES.PolicyGroup) {
    return "group";
  }
  if (configType === CONFIG_TYPES.ProxyChain) {
    return "chain";
  }

  return "";
}

function defaultPort(configType: ProfileProtocol) {
  return configType === CONFIG_TYPES.Custom || configType === CONFIG_TYPES.PolicyGroup || configType === CONFIG_TYPES.ProxyChain
    ? 0
    : 443;
}

function scrubEmptyStrings(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((item) => scrubEmptyStrings(item));
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [
        key,
        entry === "" ? undefined : scrubEmptyStrings(entry),
      ]),
    );
  }

  return value;
}
