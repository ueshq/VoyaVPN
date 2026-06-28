import type { ConfigType, CoreType, ProfileItem_Deserialize, ProfileItem_Serialize } from "@/ipc/bindings";

export const CORE_TYPES = {
  v2fly: 1,
  Xray: 2,
  v2flyV5: 4,
  mihomo: 13,
  hysteria: 21,
  naiveproxy: 22,
  tuic: 23,
  singBox: 24,
  juicity: 25,
  hysteria2: 26,
  brook: 27,
  overtls: 28,
  shadowquic: 29,
  mieru: 30,
  v2rayN: 99,
} as const satisfies Record<string, CoreType>;

export const CORE_SWITCH_OPTIONS = [
  { coreType: null, labelKey: "status.coreDefault", value: "default" },
  { coreType: CORE_TYPES.Xray, label: "Xray", value: String(CORE_TYPES.Xray) },
  { coreType: CORE_TYPES.singBox, label: "sing-box", value: String(CORE_TYPES.singBox) },
] as const;

const CONFIG_TYPES_DEFAULT_TO_SING_BOX = new Set<ConfigType>([8, 11, 12]);

export function defaultCoreTypeForConfig(configType: ConfigType | null | undefined): CoreType {
  return CONFIG_TYPES_DEFAULT_TO_SING_BOX.has(Number(configType) as ConfigType)
    ? CORE_TYPES.singBox
    : CORE_TYPES.Xray;
}

export function effectiveProfileCoreType(
  profile: ProfileItem_Deserialize | ProfileItem_Serialize,
): CoreType {
  return profile.CoreType ?? defaultCoreTypeForConfig(profile.ConfigType);
}

export function formatCoreType(coreType: CoreType | null | undefined): string {
  switch (coreType) {
    case CORE_TYPES.Xray:
      return "Xray";
    case CORE_TYPES.singBox:
      return "sing-box";
    case CORE_TYPES.mihomo:
      return "mihomo";
    default:
      return coreType == null ? "" : `Core ${coreType}`;
  }
}
