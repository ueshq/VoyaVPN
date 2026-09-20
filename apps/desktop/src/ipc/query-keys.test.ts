import { describe, expect, it } from "vitest";

import type { InvalidationScope } from "@/ipc/bindings";
import {
  connectionIpQueryKey,
  invalidationQueryKey,
  profileDetailsQueryKey,
  profileShareQrQueryKey,
  queryKeys,
} from "@/ipc/query-keys";

/**
 * The gate that stops the invalidation contract drifting again.
 *
 * Backend keys and frontend keys used to be independent string literals, and
 * they had diverged: `app-config` was emitted but never queried, seven other
 * emitted keys had no `useQuery` behind them, and `app-settings` /
 * `connection-mode` were queried but never emitted. Nothing could catch that,
 * because a bindings-drift check cannot see a string literal.
 *
 * So this test derives both sides from the code rather than from a list:
 *
 * 1. the scope variants come out of the *generated* `bindings.ts`, so a new
 *    Rust variant shows up here without anyone updating a fixture;
 * 2. the subscribed keys come out of every `useQuery` in `apps/desktop/src`.
 *
 * It then asserts the two agree, and that no `queryKey` anywhere is written as
 * a literal instead of coming from the registry.
 */
const sources = import.meta.glob("../**/*.{ts,tsx}", {
  eager: true,
  import: "default",
  query: "?raw",
}) as Record<string, string>;

// Vite normalises glob keys relative to this file, so same-directory modules
// come back as `./x.ts` and everything else as `../<dir>/x.ts`.
const bindingsSource = sources["./bindings.ts"] ?? "";

/** Every production module: tests, fixtures and the test harness excluded. */
const productionSources = Object.entries(sources).filter(
  ([path]) =>
    !/\.(test|spec)\.tsx?$/.test(path) &&
    !path.endsWith(".test-fixture.ts") &&
    !path.startsWith("../test/"),
);

/** Parameterised keys, mapped back to the root a scope can invalidate. */
const keyFactories = {
  connectionIpQueryKey: () => connectionIpQueryKey("profile:1"),
  profileDetailsQueryKey: () => profileDetailsQueryKey("profile-1"),
  profileShareQrQueryKey: () => profileShareQrQueryKey("share-link"),
};

function invalidationScopeKinds(): string[] {
  const declaration = /export type InvalidationScope\s*=([\s\S]*?);\n/.exec(bindingsSource);
  expect(declaration, "InvalidationScope missing from bindings.ts").not.toBeNull();

  const kinds = [...(declaration?.[1] ?? "").matchAll(/\{\s*kind:\s*"([A-Za-z0-9]+)"/g)].map(
    (match) => match[1] as string,
  );
  expect(kinds.length, "no InvalidationScope variants parsed").toBeGreaterThan(0);

  return kinds;
}

/** Text of every argument list passed to `useQuery(`, parens balanced. */
function findUseQueryCalls(source: string): string[] {
  const blocks: string[] = [];
  const marker = "useQuery(";
  for (let index = source.indexOf(marker); index >= 0; index = source.indexOf(marker, index + 1)) {
    let depth = 0;
    let cursor = index + marker.length - 1;
    for (; cursor < source.length; cursor += 1) {
      const character = source[cursor];
      if (character === "(") depth += 1;
      if (character === ")") {
        depth -= 1;
        if (depth === 0) break;
      }
    }
    blocks.push(source.slice(index, cursor));
  }
  return blocks;
}

/** Resolves `queryKeys.x` / `xQueryKey(...)` to the root string it produces. */
function rootOf(expression: string): string | null {
  const registryRead = /^queryKeys\.([A-Za-z0-9]+)$/.exec(expression);
  if (registryRead) {
    const entry = queryKeys[registryRead[1] as keyof typeof queryKeys] as
      | readonly string[]
      | undefined;
    return entry?.[0] ?? null;
  }

  const factoryCall = /^([A-Za-z0-9]+QueryKey)\(/.exec(expression);
  if (factoryCall) {
    const factory = keyFactories[factoryCall[1] as keyof typeof keyFactories] as
      | (() => readonly unknown[])
      | undefined;
    const root = factory ? factory()[0] : null;
    return typeof root === "string" ? root : null;
  }

  return null;
}

function queryKeyExpressions(source: string): string[] {
  return [...source.matchAll(/queryKey:\s*([^,\n]+)/g)].map((match) => (match[1] as string).trim());
}

describe("query key registry", () => {
  it("maps every generated InvalidationScope variant to a registry key", () => {
    const unmapped = invalidationScopeKinds().filter(
      (kind) => invalidationQueryKey({ kind } as InvalidationScope) === null,
    );

    expect(unmapped, "InvalidationScope variants with no query-keys.ts mapping").toEqual([]);
  });

  it("resolves every scope to a key some useQuery actually subscribes to", () => {
    const subscribed = new Set<string>();
    const unresolved: string[] = [];

    for (const [path, source] of productionSources) {
      for (const block of findUseQueryCalls(source)) {
        for (const expression of queryKeyExpressions(block)) {
          const root = rootOf(expression);
          if (root === null) {
            unresolved.push(`${path}: ${expression}`);
          } else {
            subscribed.add(root);
          }
        }
      }
    }

    // Every `useQuery` takes its key from the registry, so these roots are the
    // complete set of caches the app really reads.
    expect(unresolved, "useQuery keys not built from @/ipc/query-keys").toEqual([]);
    expect(subscribed.size).toBeGreaterThan(0);

    const orphaned = invalidationScopeKinds().filter((kind) => {
      const key = invalidationQueryKey({ kind } as InvalidationScope);
      return key === null || !subscribed.has(key[0]);
    });

    expect(orphaned, "backend scopes with no useQuery consumer").toEqual([]);
  });

  it("never writes a query key as a literal outside the registry", () => {
    const literals = productionSources
      .filter(([path]) => path !== "./query-keys.ts")
      .flatMap(([path, source]) =>
        queryKeyExpressions(source)
          .filter((expression) => expression.startsWith("["))
          .map((expression) => `${path}: ${expression}`),
      );

    expect(literals, "queryKey literals that bypass @/ipc/query-keys").toEqual([]);
  });

  it("builds every nested key on top of its registry root", () => {
    // A `profiles` invalidation has to reach the node list and the groups.
    expect(queryKeys.profileList[0]).toBe(queryKeys.profiles[0]);
    expect(queryKeys.policyGroups[0]).toBe(queryKeys.profiles[0]);
    // So is an open details dialog, which reads one node in full.
    expect(profileDetailsQueryKey("node-1")).toEqual([...queryKeys.profileDetails, "node-1"]);
    expect(queryKeys.profileDetails[0]).toBe(queryKeys.profiles[0]);
    expect(profileShareQrQueryKey("vmess://x")).toEqual([queryKeys.profileShareQr[0], "vmess://x"]);
    expect(connectionIpQueryKey("tokyo:42")).toEqual([queryKeys.connectionIp[0], "tokyo:42"]);
  });

  it("returns null for a scope this build cannot map", () => {
    // Only reachable with a stale bindings.ts, which `check:bindings` prevents;
    // the bridge relies on the null so it never throws in an event callback.
    expect(
      invalidationQueryKey({ kind: "somethingNewer" } as unknown as InvalidationScope),
    ).toBeNull();
    // Nor through a name every object inherits.
    expect(
      invalidationQueryKey({ kind: "toString" } as unknown as InvalidationScope),
    ).toBeNull();
  });
});
