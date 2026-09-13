import type { Profile } from "@/ipc/bindings";
import { CONFIG_TYPES, type ProfileProtocol as ProfileKind } from "./profile-constants";
import {
  profileFormSchema,
  activeProfileFormValues,
  type ProfileFormValues,
  type ParsedProfileFormValues,
} from "./profile-form-schema";
import { formProtocol, protocolToFormFields, protocolToFormOptions } from "./profile-form-protocol";
import { formTransport, transportToFormOptions, transportNetwork } from "./profile-form-transport";
import { formTls, tlsToFormFields } from "./profile-form-tls";
import { clean } from "./profile-form-text";

export function createDefaultProfile(
  configType: ProfileKind = CONFIG_TYPES.VMess,
): ProfileFormValues {
  return createBaseProfile(configType) as ProfileFormValues;
}

export function normalizeProfileForForm(profile: Profile): ProfileFormValues {
  const protocol = profile.protocol;
  const configType = protocol.kind;
  const server = "server" in protocol ? protocol.server : null;
  const candidate = {
    ...createBaseProfile(configType),
    indexId: profile.id,
    subscriptionId: profile.subscriptionId,
    displayLog: profile.displayLog,
    remarks: profile.remarks,
    address: server?.address ?? "",
    port: server?.port ?? 0,
    ...protocolToFormFields(protocol),
    protocolOptions: protocolToFormOptions(protocol),
    transportOptions: transportToFormOptions(profile.transport),
    ...tlsToFormFields(profile.tls),
    network: transportNetwork(profile.transport),
  };

  return candidate as ProfileFormValues;
}

export function prepareProfileForSave(
  values: ProfileFormValues | ParsedProfileFormValues,
): Profile {
  return parsedProfileToContract(
    profileFormSchema.parse(activeProfileFormValues(values)),
  );
}

function parsedProfileToContract(parsed: ParsedProfileFormValues): Profile {
  return {
    id: parsed.indexId ?? "",
    subscriptionId: clean(parsed.subscriptionId),
    displayLog: parsed.displayLog,
    remarks: parsed.remarks,
    protocol: formProtocol(parsed),
    transport: formTransport(parsed),
    tls: formTls(parsed),
  };
}

function createBaseProfile(configType: ProfileKind) {
  return {
    configType,
    indexId: "",
    subscriptionId: null,
    displayLog: true,
    remarks: "",
    address: "",
    port: 443,
    password: "",
    username: "",
    network: "tcp",
    streamSecurity: "",
    sni: "",
    alpn: "",
    publicKey: "",
    shortId: "",
    cert: "",
    echConfigList: "",
    protocolOptions: {},
    transportOptions: {},
  };
}
