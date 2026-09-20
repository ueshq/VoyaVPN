import { readFileSync } from "node:fs";
import { join } from "node:path";

import { isCliEntrypoint, repoRootFromScript } from "../lib/common.mjs";

/**
 * Packages that must resolve to exactly one version across the whole workspace.
 *
 * Two copies of a Radix primitive are not a size problem, they are a
 * correctness problem: Radix keeps portal/focus/dismiss state in module-level
 * context, so a Dialog rendered by one copy is invisible to the DismissableLayer
 * stack of the other. That produced dialogs that would not close on Escape.
 * A mixed pinning policy is what allowed it — an exact pin on
 * `@radix-ui/react-dialog` while a caret `@radix-ui/react-alert-dialog`
 * dragged in a second one — so the pins in `packages/ui/package.json` and the
 * `pnpm.overrides` block in the root manifest exist to keep this list at one
 * version each, and this gate proves they still do.
 *
 * `@tanstack/react-query` fails the same way for the same reason: the
 * `QueryClient` travels through a module-level React context, so a hook from a
 * second copy throws "No QueryClient set" inside a provider that is right
 * there. Both workspaces take it from the catalog to stay on one version.
 */
const singleVersionPackages = ["@tanstack/react-query", "react", "react-dom"];

/**
 * Scopes where *every* package must be single-version.
 *
 * Naming the primitives one by one does not work: the copies that actually
 * broke the app were transitive ones nobody had listed. Two `react-focus-scope`
 * copies ping-ponged focus between their module-level scope stacks until the
 * call stack blew, which surfaced as an out-of-memory crash in the test run,
 * not as a focus bug. Checking the whole scope catches the next one.
 */
const singleVersionScopes = ["@radix-ui/"];

/**
 * Reads the resolved package ids out of a pnpm lockfile's `packages:` (v9) or
 * `snapshots:` block. Full YAML parsing is not worth a dependency here: the
 * keys we need are the only two-space-indented quoted entries in that block.
 */
export function resolvedVersions(
  lockfileText,
  packageNames = singleVersionPackages,
  scopes = singleVersionScopes,
) {
  const wanted = new Set(packageNames);
  const found = new Map(packageNames.map((name) => [name, new Set()]));

  for (const line of lockfileText.split("\n")) {
    const match = /^ {2}'?((?:@[^/'@]+\/)?[^/'@]+)@([^'():]+)[^:]*'?:$/.exec(line);
    if (!match) continue;

    const [, name, version] = match;
    const covered = wanted.has(name) || scopes.some((scope) => name.startsWith(scope));
    if (!covered) continue;

    if (!found.has(name)) found.set(name, new Set());
    found.get(name).add(version);
  }

  return found;
}

export function checkLockfile(
  lockfileText,
  packageNames = singleVersionPackages,
  scopes = singleVersionScopes,
) {
  const failures = [];
  const report = [];

  for (const [name, versions] of resolvedVersions(lockfileText, packageNames, scopes)) {
    const sorted = [...versions].sort();
    if (sorted.length === 0) {
      failures.push(`${name} is not in the lockfile; drop it from the list or restore the dependency`);
    } else if (sorted.length > 1) {
      failures.push(`${name} resolves to ${sorted.length} versions: ${sorted.join(", ")}`);
    } else {
      report.push(`${name}: ${sorted[0]}`);
    }
  }

  return { failures, report };
}

if (isCliEntrypoint(import.meta.url)) {
  const lockfile = join(repoRootFromScript(import.meta.url), "pnpm-lock.yaml");
  const { failures, report } = checkLockfile(readFileSync(lockfile, "utf8"));
  console.log(`✓ ${report.length} packages resolve to a single version each`);

  if (failures.length > 0) {
    console.error("\nLockfile duplicate check failed:\n");
    for (const failure of failures) console.error(`- ${failure}`);
    console.error(
      "\nPin the package in packages/ui/package.json and add a pnpm.overrides" +
        "\nentry in the root package.json, then run `pnpm install`.",
    );
    process.exit(1);
  }
}
