import { z } from "zod";

import { CONFIG_TYPES } from "@voya/features/profiles/profile-constants";

/**
 * Validation failures carry locale-independent codes instead of English
 * sentences: the editor renders them through `profileValidationMessage`, so the
 * text a user sees comes from the locale system like every other visible string.
 */
export const PROFILE_VALIDATION_CODES = {
  portInvalid: "voya.profile.port.invalid",
  integerInvalid: "voya.profile.integer.invalid",
  addressRequired: "voya.profile.address.required",
  credentialRequired: "voya.profile.credential.required",
  remarksRequired: "voya.profile.remarks.required",
  uuidRequired: "voya.profile.uuid.required",
} as const;

const optionalText = z.string().optional();
const optionalNullableText = z.string().nullable().optional();
const optionalNullableBool = z.boolean().nullable().optional();
const optionalNullableNumber = z
  .number({ error: PROFILE_VALIDATION_CODES.integerInvalid })
  .int(PROFILE_VALIDATION_CODES.integerInvalid)
  .nullable()
  .optional();

const protocolOptionsSchema = z
  .object({
    udpOverTcp: optionalNullableBool,
    congestionControl: optionalNullableText,
    vmessCipher: optionalNullableText,
    flow: optionalNullableText,
    vlessEncryption: optionalNullableText,
    method: optionalNullableText,
    wireGuardPeerPublicKey: optionalNullableText,
    wireGuardPresharedKey: optionalNullableText,
    wireGuardInterfaceAddress: optionalNullableText,
    wireGuardAllowedIps: optionalNullableText,
    wireGuardReserved: optionalNullableText,
    wireGuardMtu: optionalNullableNumber,
    obfuscationPassword: optionalNullableText,
    portHops: optionalNullableText,
    insecureConcurrency: optionalNullableNumber,
    naiveQuic: optionalNullableBool,
  })
  .default({});

const transportOptionsSchema = z
  .object({
    header: optionalNullableText,
    host: optionalNullableText,
    path: optionalNullableText,
    grpcAuthority: optionalNullableText,
    grpcServiceName: optionalNullableText,
    grpcMode: optionalNullableText,
  })
  .default({});

const commonProfileSchema = z.object({
  indexId: optionalText,
  subscriptionId: optionalNullableText,
  displayLog: z.boolean().default(true),
  remarks: z.string().trim().min(1, PROFILE_VALIDATION_CODES.remarksRequired),
  address: z.string().trim().min(1, PROFILE_VALIDATION_CODES.addressRequired),
  port: z
    .number({ error: PROFILE_VALIDATION_CODES.portInvalid })
    .int(PROFILE_VALIDATION_CODES.portInvalid)
    .min(1, PROFILE_VALIDATION_CODES.portInvalid)
    .max(65535, PROFILE_VALIDATION_CODES.portInvalid),
  password: optionalText,
  username: optionalText,
  network: optionalText,
  streamSecurity: optionalText,
  sni: optionalText,
  alpn: optionalText,
  publicKey: optionalText,
  shortId: optionalText,
  cert: optionalText,
  echConfigList: optionalText,
  protocolOptions: protocolOptionsSchema,
  transportOptions: transportOptionsSchema,
});

const serverProfileSchema = commonProfileSchema.extend({
  password: z
    .string()
    .trim()
    .min(1, PROFILE_VALIDATION_CODES.credentialRequired),
});

const authProfileSchema = commonProfileSchema.extend({
  password: optionalText,
  username: optionalText,
});

// TUIC authenticates with a UUID *and* a password. The form carries the
// contract's `uuid` in `username`, and neither the backend nor sing-box rejects
// an empty uuid (it is simply omitted from the generated outbound), so the
// requirement has to be enforced here.
const tuicProfileSchema = serverProfileSchema.extend({
  username: z.string().trim().min(1, PROFILE_VALIDATION_CODES.uuidRequired),
});

export const profileFormSchema = z.discriminatedUnion("configType", [
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.VMess) }),
  serverProfileSchema.extend({
    configType: z.literal(CONFIG_TYPES.Shadowsocks),
  }),
  authProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.SOCKS) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.VLESS) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.Trojan) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.Hysteria2) }),
  tuicProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.TUIC) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.WireGuard) }),
  authProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.HTTP) }),
  serverProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.Anytls) }),
  authProfileSchema.extend({ configType: z.literal(CONFIG_TYPES.Naive) }),
]);

export type ProfileFormValues = z.input<typeof profileFormSchema>;
export type ParsedProfileFormValues = z.output<typeof profileFormSchema>;

/** Validate a copy of the active fields, retaining inactive numeric drafts in RHF. */
export function activeProfileFormValues(
  values: ProfileFormValues,
): ProfileFormValues {
  return {
    ...values,
    protocolOptions: {
      ...values.protocolOptions,
      wireGuardMtu:
        values.configType === CONFIG_TYPES.WireGuard
          ? values.protocolOptions?.wireGuardMtu
          : undefined,
      insecureConcurrency:
        values.configType === CONFIG_TYPES.Naive
          ? values.protocolOptions?.insecureConcurrency
          : undefined,
    },
  };
}
