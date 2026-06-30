import type { CoreType } from "@/ipc/bindings";

export const CORE_TYPES = {
  singBox: 24,
} as const satisfies Record<string, CoreType>;

export function formatCoreType(coreType: CoreType | null | undefined): string {
  switch (coreType) {
    case CORE_TYPES.singBox:
      return "sing-box";
    default:
      return coreType == null ? "" : `Core ${coreType}`;
  }
}
