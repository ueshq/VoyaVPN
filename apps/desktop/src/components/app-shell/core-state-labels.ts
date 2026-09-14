import type { TranslationKey } from "@voya/i18n";

import type { CoreState } from "@/ipc/bindings";

/** One wording for the connection state, shared by the sidebar footer and Home. */
export const CORE_STATE_TRANSLATION_KEYS = {
  cleanupPending: "home.cleanupPending",
  connected: "status.connected",
  connecting: "status.connecting",
  disconnected: "status.disconnected",
  disconnecting: "status.disconnecting",
} as const satisfies Record<CoreState, TranslationKey>;
