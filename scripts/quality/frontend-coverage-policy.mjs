/**
 * Which frontend modules the coverage gate actually protects.
 *
 * The first version of this policy listed nine files that were all already at
 * 97.5-100% lines, so the 80% floor could not fail for any code that existed,
 * while the global average absorbed runtime modules sitting at 0-42%. The list
 * below is rebuilt around the modules whose behaviour changes proxy/TUN state
 * or drops backend events, with two tiers:
 *
 * - `criticalModules`: fully covered contracts, all four metrics >= 80%.
 * - `runtimeModules`: a ratchet. Each floor sits roughly ten points under the
 *   coverage the module had when it was added, so the gate fails on a
 *   regression instead of only on a catastrophe. **Raise the floor whenever you
 *   raise the coverage** — that is the whole point of the tier.
 *
 * `untestedModules` is reported, never failed: those files have no meaningful
 * tests yet, and the report keeps the gap visible in every CI log and asks for
 * a promotion into `runtimeModules` once tests land.
 */

export const globalMinimums = {
  lines: 80,
  statements: 80,
  functions: 80,
  branches: 70,
};

// Kept module-local: it is only ever the default for `evaluateCoverage`.
const criticalMinimum = 80;

export const criticalModules = [
  "apps/desktop/src/ipc/commands.ts",
  "apps/desktop/src/ipc/runtime-state-version.ts",
  "apps/desktop/src/ipc/runtime-status.ts",
  "apps/desktop/src/ipc/process.ts",
  "apps/desktop/src/ipc/updater.ts",
  "apps/desktop/src/ipc/notifications.ts",
  "apps/desktop/src/features/settings/use-app-settings.ts",
  "apps/desktop/src/features/dns/use-dns-settings.ts",
  "apps/desktop/src/features/profiles/profile-form-schema.ts",
  "apps/desktop/src/features/profiles/profile-form-protocol.ts",
  "apps/desktop/src/features/profiles/profile-form-transport.ts",
  "apps/desktop/src/features/profiles/profile-form-tls.ts",
  "apps/desktop/src/features/profiles/profile-form-text.ts",
  "apps/desktop/src/features/profiles/virtual-list-keyboard.ts",
  "apps/desktop/src/features/routing/per-app-proxy-rule.ts",
  "apps/desktop/src/features/routing/routing-form-schema.ts",
  "apps/desktop/src/features/routing/routing-form-values.ts",
  "apps/desktop/src/features/routing/routing-rule-dialog.tsx",
  "apps/desktop/src/features/routing/use-routing-screen.ts",
  "apps/desktop/src/stores/shell-store.ts",
  "packages/utils/src/formatting.ts",
  "packages/utils/src/operational-redaction.ts",
];

export const runtimeModules = [
  // The single mounted bridge for all three ADR-0002 event channels.
  { path: "apps/desktop/src/ipc/event-bridge.tsx", lines: 65, branches: 45 },
  { path: "apps/desktop/src/ipc/runtime-event-store.ts", lines: 65, branches: 50 },
  // Connect/disconnect on the home screen.
  { path: "apps/desktop/src/features/home/use-home-runtime.ts", lines: 65, branches: 50 },
  { path: "apps/desktop/src/features/home/runtime-action.ts", lines: 50, branches: 25 },
  // Shared activation retains the guards and recovery paths extracted from Home.
  { path: "apps/desktop/src/features/home/use-profile-activation.ts", lines: 80, branches: 75 },
  { path: "apps/desktop/src/stores/runtime-action-store.ts", lines: 90, branches: 90 },
  // Proxy-monitor lifecycle.
  { path: "apps/desktop/src/components/app-shell/app-shell.tsx", lines: 75, branches: 55 },
  { path: "apps/desktop/src/features/profiles/use-server-table.ts", lines: 75, branches: 55 },
  // Share-link-only export: 83/78 when the Base64, bundle and file-save paths were removed.
  { path: "apps/desktop/src/features/profiles/server-table-actions.ts", lines: 75, branches: 65 },
  { path: "apps/desktop/src/features/profiles/use-node-groups.ts", lines: 80, branches: 65 },
  { path: "apps/desktop/src/features/profiles/node-list-rows.ts", lines: 80, branches: 65 },
  // Page composition keeps the old floor; each extracted capability has its own floor.
  { path: "apps/desktop/src/features/profiles/use-node-list-data.ts", lines: 90, branches: 75 },
  { path: "apps/desktop/src/features/profiles/use-node-editor.ts", lines: 85, branches: 65 },
  { path: "apps/desktop/src/features/profiles/use-node-export.ts", lines: 80, branches: 75 },
  { path: "apps/desktop/src/features/profiles/use-node-operation.ts", lines: 90, branches: 90 },
  { path: "apps/desktop/src/features/profiles/use-node-subscriptions.ts", lines: 80, branches: 75 },
  { path: "apps/desktop/src/features/profiles/use-node-speedtest.ts", lines: 90, branches: 50 },
  { path: "apps/desktop/src/features/profiles/profile-form-values.ts", lines: 90, branches: 60 },
  // TLS field panel in the profile dialog; certificate fetch/hash was retired.
  { path: "apps/desktop/src/features/profiles/profile-security-panel.tsx", lines: 85, branches: 70 },
  { path: "apps/desktop/src/features/updates/app-update-flow.ts", lines: 85, branches: 75 },
  { path: "apps/desktop/src/stores/preferences-store.ts", lines: 80, branches: 55 },
  { path: "apps/desktop/src/stores/toast-store.ts", lines: 70, branches: 80 },
  // Promoted out of `untestedModules` once the typed-contract passes gave them
  // real tests. Measured at promotion: modal-host 92/80, modal-store 86/100.
  { path: "apps/desktop/src/components/app-shell/modal-host.tsx", lines: 82, branches: 70 },
  // The Rules page: one active rule set, a sortable rule list, its dialogs and
  // the traffic mode that locks them in global mode.
  { path: "apps/desktop/src/features/routing/routing-rule-list.tsx", lines: 90, branches: 90 },
  { path: "apps/desktop/src/features/routing/routing-screen.tsx", lines: 90, branches: 90 },
  { path: "apps/desktop/src/features/routing/traffic-mode-switcher.tsx", lines: 85, branches: 75 },
  { path: "apps/desktop/src/stores/modal-store.ts", lines: 75, branches: 90 },
];

/**
 * Modules with no meaningful tests. Reported so the gap stays visible; promote
 * one into `runtimeModules` (with a floor) as soon as it gets tests.
 */
export const untestedModules = [];

const METRICS = ["lines", "functions", "branches", "statements"];

function percent(entry, metric) {
  const value = entry?.[metric]?.pct;
  return typeof value === "number" ? value : null;
}

/**
 * @param {(relativePath: string) => object | undefined} lookup
 *   Resolves a repository-relative path to its coverage-summary entry.
 */
export function evaluateCoverage({ total, lookup, policy = {} }) {
  const {
    globals = globalMinimums,
    critical = criticalModules,
    runtime = runtimeModules,
    untested = untestedModules,
    criticalFloor = criticalMinimum,
  } = policy;

  const failures = [];
  const warnings = [];

  for (const metric of METRICS) {
    const actual = percent(total, metric);
    const minimum = globals[metric];
    if (typeof minimum !== "number") continue;
    if (actual === null || actual < minimum) {
      failures.push(`total: ${metric} ${actual ?? "missing"}% < ${minimum}%`);
    }
  }

  for (const relativePath of critical) {
    const entry = lookup(relativePath);
    if (!entry) {
      failures.push(`${relativePath}: missing from the coverage report`);
      continue;
    }
    for (const metric of METRICS) {
      const actual = percent(entry, metric);
      if (actual === null || actual < criticalFloor) {
        failures.push(`${relativePath}: ${metric} ${actual ?? "missing"}% < ${criticalFloor}%`);
      }
    }
  }

  for (const module of runtime) {
    const entry = lookup(module.path);
    if (!entry) {
      failures.push(`${module.path}: missing from the coverage report`);
      continue;
    }
    for (const metric of ["lines", "branches"]) {
      const minimum = module[metric];
      if (typeof minimum !== "number") continue;
      const actual = percent(entry, metric);
      if (actual === null || actual < minimum) {
        failures.push(`${module.path}: ${metric} ${actual ?? "missing"}% < ${minimum}% (runtime floor)`);
      }
    }
  }

  for (const module of untested) {
    const entry = lookup(module.path);
    if (!entry) {
      warnings.push(`${module.path}: still absent from the coverage report`);
      continue;
    }
    const actual = percent(entry, "lines") ?? 0;
    warnings.push(
      actual >= module.promoteAbove
        ? `${module.path}: now at ${actual}% lines — move it into runtimeModules with a floor`
        : `${module.path}: ${actual}% lines, still untested`,
    );
  }

  return { failures, warnings };
}
