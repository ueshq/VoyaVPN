import type { ConfigType, CoreType, ProfileItem_Deserialize, ProfileItem_Serialize } from "@/ipc/bindings";

export const CORE_TYPES = {
  singBox: 24,
} as const satisfies Record<string, CoreType>;

export const CORE_SWITCH_OPTIONS = [
  { coreType: null, labelKey: "status.coreDefault", value: "default" },
  { coreType: CORE_TYPES.singBox, label: "sing-box", value: String(CORE_TYPES.singBox) },
] as const;

export function defaultCoreTypeForConfig(configType: ConfigType | null | undefined): CoreType {
  void configType;

  return CORE_TYPES.singBox;
}

export function effectiveProfileCoreType(
  profile: ProfileItem_Deserialize | ProfileItem_Serialize,
): CoreType {
  return profile.CoreType ?? defaultCoreTypeForConfig(profile.ConfigType);
}

export function formatCoreType(coreType: CoreType | null | undefined): string {
  switch (coreType) {
    case CORE_TYPES.singBox:
      return "sing-box";
    default:
      return coreType == null ? "" : `Core ${coreType}`;
  }
}
