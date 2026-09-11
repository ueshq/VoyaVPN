/**
 * The one place a TanStack Query key is written down.
 *
 * ADR-0002 channel 1 says the backend owns cache invalidation, but the keys
 * used to be free strings on both sides with nothing tying them together — and
 * they had drifted: the shell emitted `app-config` while the only settings
 * query was keyed `app-settings`, seven emitted keys had no `useQuery` behind
 * them at all, and features compensated with two dozen manual
 * `invalidateQueries` calls.
 *
 * Now the vocabulary is the generated `InvalidationScope` union and this
 * module is its only translation: {@link invalidationQueryKey} maps every
 * backend variant onto the key its `useQuery` really uses, and every `useQuery`
 * in the app takes its key from {@link queryKeys} instead of writing a literal.
 * Adding a variant in `voya-contracts` fails the typecheck here until it is
 * mapped, and `query-keys.test.ts` enumerates the variants straight out of
 * `bindings.ts` so neither half can rot.
 */
import type { InvalidationScope } from "./bindings";

/**
 * Key roots, one per cache. Invalidating a root also invalidates every
 * parameterised key built from it — that is how one `profiles` scope reaches
 * all of {@link profilesQueryKey}'s filter slices.
 *
 * `processCandidates` and `profileShareQr` deliberately have no
 * `InvalidationScope`: nothing the backend commits changes a running-process
 * enumeration or a QR rendering, so no emitter could ever name them.
 */
export const queryKeys = {
  appSettings: ["app-settings"],
  settingsApply: ["app-settings", "apply-status"],
  connectionMode: ["connection-mode"],
  dns: ["dns"],
  processCandidates: ["process-candidates"],
  profileShareQr: ["profile-share-qr"],
  profiles: ["profiles"],
  proxyConnections: ["proxy-connections"],
  routings: ["routings"],
  subscriptionMetadata: ["subscription-metadata"],
  subscriptions: ["subscriptions"],
  uiPreferences: ["ui-preferences"],
} as const;

export type QueryKeyRoot = (typeof queryKeys)[keyof typeof queryKeys];

/** One filter slice of the profile list. */
export function profilesQueryKey(filter: string) {
  return [...queryKeys.profiles, { filter }] as const;
}

/** The rendered QR for one share link. */
export function profileShareQrQueryKey(content: string) {
  return [...queryKeys.profileShareQr, content] as const;
}

/**
 * Resolves a backend invalidation scope to the key the event bridge should
 * invalidate.
 *
 * Returns `null` only for a scope this build does not know, which cannot happen
 * while `bindings.ts` is in sync — `pnpm check:bindings` is a CI gate — so the
 * bridge skips it rather than throwing inside a Tauri event callback.
 */
export function invalidationQueryKey(
  scope: InvalidationScope,
): QueryKeyRoot | null {
  switch (scope.kind) {
    case "appSettings":
      return queryKeys.appSettings;
    case "connectionMode":
      return queryKeys.connectionMode;
    case "dns":
      return queryKeys.dns;
    case "profiles":
      return queryKeys.profiles;
    case "proxyConnections":
      return queryKeys.proxyConnections;
    case "routings":
      return queryKeys.routings;
    case "subscriptionMetadata":
      return queryKeys.subscriptionMetadata;
    case "subscriptions":
      return queryKeys.subscriptions;
    case "uiPreferences":
      return queryKeys.uiPreferences;
    default:
      // `satisfies never` is the exhaustiveness check: a new variant in
      // `voya-contracts` stops compiling here until the switch maps it.
      scope satisfies never;
      return null;
  }
}
