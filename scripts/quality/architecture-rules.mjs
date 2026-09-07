import { dirname, resolve } from "node:path";

/**
 * Pattern-based architecture rules, kept apart from the file walk in
 * `architecture.mjs` so every bypass this gate has ever had can be pinned by a
 * unit test in `architecture-rules.test.mjs`.
 *
 * Each rule is `{ id, pattern, message }`. `pattern` is matched against a Rust
 * file's *production* source (test modules already stripped) unless the rule
 * carries `scope: "raw"`.
 */

/**
 * `#[cfg(...)]` and `cfg!(...)` in every shape that names a platform, not just
 * the bare `#[cfg(target_os = ...)]` the first version of this gate looked for:
 * `#[cfg(any(target_os = "x", ...))]`, `#[cfg(windows)]`, `#[cfg(unix)]`,
 * `#[cfg(target_family = "...")]` and `#[cfg_attr(...)]` all pass otherwise.
 * A character class of `[^\]]` spans newlines, so multi-line attributes match.
 */
const PLATFORM_CFG_ATTRIBUTE =
  /#\[cfg(?:_attr)?\s*\([^\]]*\b(?:target_os|target_family|target_arch|target_vendor|windows|unix)\b/u;

const PLATFORM_CFG_MACRO = /\bcfg!\s*\([^)]*\b(?:target_os|target_family|target_arch|windows|unix)\b/u;

/**
 * A `use std::{...}` / `use tokio::{...}` group hides a banned module from a
 * contiguous-token match: `use std::{fs, io};` never contains the substring
 * `std::fs`. The statement is bounded by its `;`, so `[^;]*` cannot run away.
 */
function groupedImport(crateName, modules) {
  return new RegExp(`\\buse\\s+${crateName}::\\{[^;]*?\\b(?:${modules})\\b`, "u");
}

export const voyaCoreRules = [
  {
    id: "core-os-cfg",
    pattern: PLATFORM_CFG_ATTRIBUTE,
    message: "voya-core must be OS independent",
  },
  {
    id: "core-os-cfg-macro",
    pattern: PLATFORM_CFG_MACRO,
    message: "voya-core must be OS independent (cfg! on a platform predicate)",
  },
  {
    // `std::net::IpAddr` and friends are plain value types a pure domain crate
    // may parse and compare; only the socket I/O types are banned.
    id: "core-os-api",
    pattern:
      /\bstd::(?:fs|env|process)\b|\bstd::net::(?:TcpStream|TcpListener|UdpSocket|ToSocketAddrs)\b|\btokio::(?:fs|net|process|time)\b|\breqwest\b/u,
    message: "voya-core must not touch the filesystem, network, environment, or processes",
  },
  {
    id: "core-os-api-grouped",
    pattern: groupedImport("std", "fs|env|process"),
    message: "voya-core must not touch the filesystem, network, environment, or processes",
  },
  {
    id: "core-nondeterminism",
    pattern: /\bSystemTime::now\s*\(|\bInstant::now\s*\(|\brand::|\buse\s+rand\b/u,
    message: "voya-core must receive clocks and randomness through injected ports",
  },
  {
    id: "core-specta",
    pattern: /\bspecta\b/u,
    message: "voya-core must not depend on IPC type generation",
  },
];

export const voyaAppRules = [
  {
    // `std::process::id()` (a PID read) and `std::net::SocketAddr` (a value
    // type) stay allowed; spawning, HTTP, and filesystem access do not.
    id: "app-io-adapters",
    pattern:
      /\b(?:reqwest|tokio_tungstenite|tokio::net|tokio::fs|tokio::process|std::fs)\b|\bstd::process::(?:Command|Stdio)\b/u,
    message: "voya-app must use network and filesystem adapters",
  },
  {
    id: "app-io-adapters-grouped",
    pattern: groupedImport("std", "fs"),
    message: "voya-app must use network and filesystem adapters",
  },
  {
    id: "app-io-adapters-grouped-tokio",
    pattern: groupedImport("tokio", "fs|net|process"),
    message: "voya-app must use network and filesystem adapters",
  },
  {
    id: "app-specta",
    pattern: /\bspecta\b/u,
    message: "voya-app must expose IPC through voya-contracts",
  },
];

export const shellRules = [
  {
    id: "shell-tests",
    scope: "raw",
    pattern: /#\[cfg\(test\)\]/u,
    message: "tests belong in app/contracts/platform crates because the Tauri shell lib harness is disabled",
  },
  {
    id: "shell-domain",
    pattern: /\bvoya_(?:core|db)::/u,
    message: "the Tauri shell must reach domain and persistence through voya-app facades",
  },
];

export const shellDtoRule = {
  id: "shell-dto",
  pattern: /#\[derive\([^\]]*\bType\b/u,
  message: "business and command DTOs belong in voya-contracts",
};

export const retiredCompatibilityRules = [
  {
    id: "retired-share-scheme",
    pattern: /v2rayn:\/\//iu,
    message: "the private v2rayn:// format is retired",
  },
  {
    id: "retired-config-compat",
    pattern:
      /\b(?:AppConfigStore|ProtocolExtraItem|TransportExtraItem|remove_retired_voya_config_fields|prev_profile|next_profile)\b/u,
    message: "retired configuration compatibility code is forbidden",
  },
];

export const clashBoundaryRules = [
  {
    id: "retired-serde-alias",
    pattern: /serde\([^\n]*\balias\s*=/u,
    message: "serde aliases are retired outside the documented Clash API boundary",
  },
  {
    id: "retired-pascal-case",
    pattern: /rename_all\s*=\s*"PascalCase"/u,
    message: "v2rayN-style PascalCase serialization is retired outside the Clash API boundary",
  },
];

export const contractsCasingRule = {
  id: "contracts-casing",
  pattern: /rename_all\s*=\s*"PascalCase"/u,
  message: "public contracts must use camelCase",
};

/**
 * Manifest dependency bans. A plain `^name =` match is defeated by Cargo's two
 * other spellings of the same edge: a renamed dependency
 * (`domain = { package = "voya-core", ... }`) and dependency-table syntax
 * (`[dependencies.voya-core]`, including `[target.'cfg(...)'.dependencies.…]`).
 */
export function manifestDependencyRules(cratePattern, message) {
  return [
    { id: "manifest-direct", pattern: new RegExp(`^[ \\t]*${cratePattern}(?:\\.workspace)?\\s*=`, "mu"), message },
    { id: "manifest-renamed", pattern: new RegExp(`\\bpackage\\s*=\\s*"${cratePattern}"`, "u"), message },
    {
      id: "manifest-table",
      pattern: new RegExp(`^[ \\t]*\\[[^\\]]*dependencies\\.${cratePattern}\\]`, "mu"),
      message,
    },
  ];
}

/**
 * `unsafe` sites that need a nearby `SAFETY:` rationale.
 *
 * The original regex matched only `unsafe {` and `unsafe impl`, which left the
 * two shapes an FFI crate actually uses uncovered: `unsafe extern "C" { … }`
 * (whose signatures are the safety contract) and `unsafe fn` declarations. The
 * workspace now also turns on rustc's `unsafe_op_in_unsafe_fn`, which edition
 * 2021 leaves allow-by-default, so unsafe work inside an `unsafe fn` needs an
 * inner block that clippy's `undocumented_unsafe_blocks` can demand a comment
 * for; this rule still covers the signature line itself.
 */
const UNSAFE_SITE = /\bunsafe\s+(?:extern\b|fn\b)|\bunsafe\s*(?:\{|impl\b)/u;

/**
 * Escape hatch for `unsafe` sites that cannot carry a `// SAFETY:` comment yet.
 * Entries are matched on the exact trimmed line, so they cannot suppress a
 * different `unsafe` site in the same file, and `architecture.mjs` reports an
 * entry that no longer matches anything so it gets deleted again.
 *
 * Deliberately empty: every `unsafe` site in the workspace documents itself.
 */
export const KNOWN_UNSAFE_WITHOUT_SAFETY_COMMENT = [];

export function findUndocumentedUnsafe(source, { path = "", allowlist = KNOWN_UNSAFE_WITHOUT_SAFETY_COMMENT } = {}) {
  const lines = source.split(/\r?\n/u);
  const findings = [];
  const usedAllowlistEntries = new Set();

  for (let index = 0; index < lines.length; index += 1) {
    if (!UNSAFE_SITE.test(lines[index])) continue;
    if (/SAFETY:/u.test(lines.slice(Math.max(0, index - 3), index).join("\n"))) continue;

    const trimmed = lines[index].trim();
    const allowed = allowlist.findIndex((entry) => entry.path === path && entry.line === trimmed);
    if (allowed !== -1) {
      usedAllowlistEntries.add(allowed);
      continue;
    }

    findings.push({ line: index + 1, text: trimmed });
  }

  return { findings, usedAllowlistEntries };
}

/**
 * Resolves the files that are declared as `#[cfg(test)] mod <name>;` somewhere
 * in the workspace.
 *
 * The gate used to exempt any file *named* `tests.rs` or `golden.rs` from every
 * rule, which is a bypass anyone can take by naming a production module
 * `tests.rs`. Exempting the declared test modules instead means the exemption
 * follows the `#[cfg(test)]` attribute, not the filename.
 */
export function resolveTestModuleFiles({ files, read }) {
  const known = new Set(files);
  const testModuleFiles = new Set();
  const declaration = /#\[cfg\(test\)\]\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;/gu;

  for (const file of files) {
    const source = read(file);
    for (const match of source.matchAll(declaration)) {
      for (const candidate of moduleFileCandidates(file, match[1])) {
        if (known.has(candidate)) {
          testModuleFiles.add(candidate);
        }
      }
    }
  }

  return testModuleFiles;
}

/**
 * Cargo's module file lookup: a child of `lib.rs`/`main.rs`/`mod.rs` lives next
 * to it, a child of `foo.rs` lives in the sibling `foo/` directory.
 */
export function moduleFileCandidates(declaringFile, moduleName) {
  const directory = dirname(declaringFile);
  const stem = declaringFile.replace(/\.rs$/u, "");
  const isDirectoryOwner = /(?:^|[\\/])(?:lib|main|mod)\.rs$/u.test(declaringFile);
  const base = isDirectoryOwner ? directory : stem;

  return [resolve(base, `${moduleName}.rs`), resolve(base, moduleName, "mod.rs")];
}
