import { describe, expect, it } from "vitest";

import {
  cargoPackageVersion,
  cargoWorkspaceMembers,
  clashBoundaryRules,
  contractsCasingRule,
  findUndocumentedUnsafe,
  KNOWN_UNSAFE_WITHOUT_SAFETY_COMMENT,
  manifestDependencyRules,
  moduleFileCandidates,
  resolveTestModuleFiles,
  retiredCompatibilityRules,
  shellDtoRule,
  shellRules,
  untranslatedMessageRule,
  gradleVersionName,
  versionAlignmentProblem,
  xcodeMarketingVersion,
  voyaAppRules,
  voyaCoreRules,
} from "./architecture-rules.mjs";

/** Every bypass below used to satisfy the gate; each case pins one of them. */
function violates(rules, source) {
  return rules.filter((rule) => rule.pattern.test(source)).map((rule) => rule.id);
}

describe("voya-core OS independence", () => {
  it.each([
    ['#[cfg(target_os = "windows")]', "core-os-cfg"],
    ['#[cfg(any(target_os = "macos", target_os = "ios"))]', "core-os-cfg"],
    ["#[cfg(windows)]", "core-os-cfg"],
    ["#[cfg(unix)]", "core-os-cfg"],
    ['#[cfg(target_family = "unix")]', "core-os-cfg"],
    ['#[cfg_attr(target_os = "linux", derive(Debug))]', "core-os-cfg"],
    ['#[cfg(\n    any(target_os = "linux", target_os = "android")\n)]', "core-os-cfg"],
    ['if cfg!(target_os = "windows") { 1 } else { 2 }', "core-os-cfg-macro"],
    ["if cfg!(windows) { 1 } else { 2 }", "core-os-cfg-macro"],
  ])("rejects %s", (source, id) => {
    expect(violates(voyaCoreRules, source)).toContain(id);
  });

  it.each([
    ["use std::fs::File;", "core-os-api"],
    ["let value = std::env::var(\"HOME\");", "core-os-api"],
    ["std::process::Command::new(\"sing-box\")", "core-os-api"],
    ["let listener = std::net::TcpListener::bind(addr)?;", "core-os-api"],
    // Grouped and module-relative socket imports never spell `std::net::TcpStream`.
    ["use std::net::{TcpStream, UdpSocket};", "core-os-api"],
    ["use std::{collections::BTreeMap, net::{IpAddr, TcpListener}};", "core-os-api"],
    ["use std::net;\nlet stream = net::TcpStream::connect(addr)?;", "core-os-api"],
    ["use std::net::*;\nlet addrs = host.to_socket_addrs()?;", "core-os-api"],
    ["use tokio::fs;", "core-os-api"],
    ["use reqwest::Client;", "core-os-api"],
    ["use std::{collections::BTreeMap, fs};", "core-os-api-grouped"],
    ["use std::{\n    collections::BTreeMap,\n    env,\n};", "core-os-api-grouped"],
    ["let now = SystemTime::now();", "core-nondeterminism"],
    ["let started = Instant::now();", "core-nondeterminism"],
    ["let value = rand::random::<u16>();", "core-nondeterminism"],
    ["use specta::Type;", "core-specta"],
  ])("rejects %s", (source, id) => {
    expect(violates(voyaCoreRules, source)).toContain(id);
  });

  it("still allows the pure value types and injected ports a domain crate needs", () => {
    for (const source of [
      "use std::{collections::BTreeMap, net::IpAddr};",
      "use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};",
      "use std::net::{IpAddr, SocketAddrV4};",
      "use std::time::Duration;",
      "#[cfg(test)]\nmod tests;",
      "#[cfg_attr(test, derive(Debug))]",
      "let elapsed = clock.now();",
    ]) {
      expect(violates(voyaCoreRules, source), source).toEqual([]);
    }
  });
});

describe("voya-app adapter boundary", () => {
  it.each([
    ["use reqwest::Client;", "app-io-adapters"],
    ["use tokio::net::TcpStream;", "app-io-adapters"],
    ["std::process::Command::new(\"sudo\")", "app-io-adapters"],
    ["use tokio::process::Command;", "app-io-adapters"],
    // The grouped form: `use std::{fs, io};` contains no `std::fs` substring.
    ["use std::{fs, io};", "app-io-adapters-grouped"],
    ["use std::{\n    fs,\n    path::PathBuf,\n};", "app-io-adapters-grouped"],
    ["use std::{sync::Arc, fs::File};", "app-io-adapters-grouped"],
    ["use tokio::{sync::Mutex, fs};", "app-io-adapters-grouped-tokio"],
    // Spawning through an import that never spells `std::process::Command`.
    ["use std::process::{Command, Stdio};", "app-io-adapters-process-import"],
    ["use std::{io, process::Command};", "app-io-adapters-process-import"],
    ["use std::{\n    io,\n    process::{Command, Stdio},\n};", "app-io-adapters-process-import"],
    ["use std::process::Command as Spawn;", "app-io-adapters-process-import"],
    ["use std::process;\nprocess::Command::new(\"sudo\")", "app-io-adapters-process-import"],
    ["use std::{process, io};\nprocess::Command::new(\"sudo\")", "app-io-adapters-process-import"],
    ["use std::{io, process};", "app-io-adapters-process-import"],
    ["use std::{io::{self, Write}, process};", "app-io-adapters-process-import"],
    ["use std::process::{self, ExitCode};", "app-io-adapters-process-import"],
    ["use std::process as proc;", "app-io-adapters-process-import"],
    ["use specta::Type;", "app-specta"],
  ])("rejects %s", (source, id) => {
    expect(violates(voyaAppRules, source)).toContain(id);
  });

  it("allows PID reads, address value types, and unrelated grouped imports", () => {
    for (const source of [
      "let pid = u128::from(std::process::id());",
      "std::process::exit(0);",
      "use std::process::id;",
      "use std::{io, process::ExitCode};",
      "use std::os::unix::process::CommandExt;",
      "use std::{io, os::unix::process::CommandExt};",
      // voya-app's own adapters live in modules that are also called `process`.
      "use voya_platform::{coreinfo::TargetOs, process::{ProcessError, ProcessRunner}};",
      "use super::{process::core_executable, SelfHostDeps};",
      "use std::net::{SocketAddr, TcpListener};",
      "use std::{sync::Arc, time::Duration};",
      "use tokio::{sync::Mutex, time::sleep};",
    ]) {
      expect(violates(voyaAppRules, source), source).toEqual([]);
    }
  });
});

describe("Tauri shell boundary", () => {
  it("rejects shell tests and direct domain access", () => {
    expect(violates(shellRules, "#[cfg(test)]\nmod tests {}")).toContain("shell-tests");
    expect(violates(shellRules, "use voya_core::AppConfig;")).toContain("shell-domain");
    expect(violates(shellRules, "voya_db::ProfileRepository::new(pool)")).toContain("shell-domain");
  });

  it.each([
    '#[cfg(all(test, feature = "x"))]\nmod tests {}',
    "#[cfg( test )]\nmod tests {}",
    "#[cfg(any(debug_assertions, test))]\nfn helper() {}",
  ])("rejects test-only code behind %s", (source) => {
    expect(violates(shellRules, source)).toContain("shell-tests");
  });

  it.each([
    "#[cfg(not(test))]\nfn production() {}",
    "#[cfg(not( test ))]\nfn production() {}",
    '#[cfg(feature = "test-utils")]',
    '#[cfg(feature = "unit_test")]',
  ])("accepts %s", (source) => {
    expect(violates(shellRules, source)).toEqual([]);
  });

  it.each([
    "#[derive(Debug, Serialize, Type)]",
    "#[derive(specta::Type)]",
    "#[derive(\n    Debug,\n    Type,\n)]",
    // Wrapping the derive in cfg_attr compiles the same DTO in the shell.
    '#[cfg_attr(feature = "ipc", derive(specta::Type))]',
  ])("rejects a shell DTO derived as %s", (source) => {
    expect(violates([shellDtoRule], source)).toContain("shell-dto");
  });

  it.each([
    "#[derive(Debug, Clone)]\nstruct TypeName;",
    "#[derive(Serialize)]\npub struct Type;",
    "#[cfg_attr(test, derive(Debug))]",
  ])("accepts %s", (source) => {
    expect(violates([shellDtoRule], source)).toEqual([]);
  });
});

describe("retired v2rayN compatibility", () => {
  it.each([
    ['const SCHEME: &str = "v2rayn://";', "retired-share-scheme"],
    ['if url.starts_with("V2RAYN://") {}', "retired-share-scheme"],
    ["struct AppConfigStore;", "retired-config-compat"],
    ["let extra = ProtocolExtraItem::default();", "retired-config-compat"],
    ["fn remove_retired_voya_config_fields() {}", "retired-config-compat"],
    ["let id = config.prev_profile;", "retired-config-compat"],
  ])("rejects %s", (source, id) => {
    expect(violates(retiredCompatibilityRules, source)).toContain(id);
  });

  it.each([
    "struct AppConfig;",
    "let prev_profile_id = String::new();",
    'const SCHEME: &str = "vless://";',
  ])("accepts %s", (source) => {
    expect(violates(retiredCompatibilityRules, source)).toEqual([]);
  });
});

describe("contracts casing", () => {
  it.each([
    '#[serde(rename_all = "PascalCase")]',
    '#[serde(rename_all = "snake_case")]',
    '#[serde(rename_all = "lowercase")]',
    '#[serde(tag = "type", rename_all_fields = "kebab-case")]',
  ])("rejects %s", (source) => {
    expect(violates([contractsCasingRule], source)).toContain("contracts-casing");
  });

  it.each([
    '#[serde(rename_all = "camelCase")]',
    '#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]',
    '#[serde(rename = "HTTP")]',
  ])("accepts %s", (source) => {
    expect(violates([contractsCasingRule], source)).toEqual([]);
  });
});

describe("Clash boundary rules", () => {
  it("rejects retired serde compatibility outside clash.rs", () => {
    expect(violates(clashBoundaryRules, '#[serde(alias = "HeaderType")]')).toContain("retired-serde-alias");
    expect(violates(clashBoundaryRules, '#[serde(rename_all = "PascalCase")]')).toContain("retired-pascal-case");
  });
});

describe("untranslated message escape hatch", () => {
  it.each([
    'ValidationIssue::untranslated("pem", error.to_string())',
    "let code = ValidationCode::Untranslated { message };",
    "vec![ValidationIssue::untranslated(field, message)]",
  ])("rejects %s outside contract_map/errors.rs", (source) => {
    expect(violates([untranslatedMessageRule], source)).toContain("untranslated-message");
  });

  it.each([
    "ValidationIssue::new(field, ValidationCode::InvalidPort)",
    "let code = ValidationCode::DnsAddressEmpty;",
    "// Untranslated text is only allowed in the error mapper.",
  ])("accepts %s", (source) => {
    expect(violates([untranslatedMessageRule], source)).toEqual([]);
  });
});

describe("manifest dependency rules", () => {
  const rules = manifestDependencyRules(
    "voya-(?:core|db)",
    "the Tauri shell must not depend directly on voya-core or voya-db",
  );

  it.each([
    ['voya-core = { path = "../../../crates/voya-core" }', "manifest-direct"],
    ["voya-db.workspace = true", "manifest-direct"],
    ["  voya-core.workspace = true", "manifest-direct"],
    // A renamed dependency hides the crate name from a line-anchored match.
    ['domain = { package = "voya-core", path = "../../../crates/voya-core" }', "manifest-renamed"],
    ["[dependencies.voya-core]\npath = \"../../../crates/voya-core\"", "manifest-table"],
    ["[dev-dependencies.voya-db]", "manifest-table"],
    ["[target.'cfg(windows)'.dependencies.voya-core]", "manifest-table"],
  ])("rejects %s", (source, id) => {
    expect(violates(rules, source)).toContain(id);
  });

  it("allows the contracts crate and unrelated names", () => {
    expect(violates(rules, "voya-contracts.workspace = true")).toEqual([]);
    expect(violates(rules, 'voya-core-utils = "1.0"')).toEqual([]);
  });

  it("rejects a renamed or tabled specta dependency in voya-app", () => {
    const spectaRules = manifestDependencyRules("specta", "voya-app must not depend on Specta");
    expect(violates(spectaRules, 'types = { package = "specta", version = "2" }')).toContain("manifest-renamed");
    expect(violates(spectaRules, "[dependencies.specta]")).toContain("manifest-table");
  });

  it("rejects a network client in voya-app under any spelling", () => {
    const networkRules = manifestDependencyRules(
      "(?:reqwest|tokio-tungstenite)",
      "voya-app must reach the network through voya-net",
    );
    expect(violates(networkRules, 'reqwest = { version = "0.12" }')).toContain("manifest-direct");
    // Renamed, the source rules would only ever see `http::Client`.
    expect(violates(networkRules, 'http = { package = "reqwest", version = "0.12" }')).toContain(
      "manifest-renamed",
    );
    expect(violates(networkRules, "[dependencies.tokio-tungstenite]")).toContain("manifest-table");
    expect(violates(networkRules, "voya-net.workspace = true")).toEqual([]);
  });
});

describe("release version alignment", () => {
  const workspaceManifest = [
    "[workspace]",
    "members = [",
    '    "crates/voya-core",',
    '    "apps/desktop/src-tauri",',
    "]",
    "",
    "[workspace.package]",
    'edition = "2021"',
    'version = "0.4.0"',
    "",
    "[workspace.dependencies]",
    'serde = { version = "1.0" }',
  ].join("\n");

  it("lists the workspace members", () => {
    expect(cargoWorkspaceMembers(workspaceManifest)).toEqual(["crates/voya-core", "apps/desktop/src-tauri"]);
  });

  it("resolves an inherited or an own Cargo package version", () => {
    expect(cargoPackageVersion('[package]\nname = "voyavpn"\nversion.workspace = true\n', workspaceManifest)).toBe(
      "0.4.0",
    );
    expect(cargoPackageVersion('[package]\nversion = { workspace = true }\n', workspaceManifest)).toBe("0.4.0");
    expect(cargoPackageVersion('[package]\nname = "voyavpn"\nversion = "0.3.9"\n', workspaceManifest)).toBe("0.3.9");
    // A dependency's `version` is not the package's.
    expect(cargoPackageVersion('[package]\nname = "x"\n\n[dependencies]\nserde = { version = "1" }\nversion = "9"\n', workspaceManifest))
      .toBeUndefined();
  });

  it("accepts one version everywhere", () => {
    expect(versionAlignmentProblem([["package.json", "0.4.0"], ["tauri.conf.json", "0.4.0"], ["Cargo", "0.4.0"]])).toBeNull();
  });

  it("names every location when one of them drifts or is missing", () => {
    const drift = versionAlignmentProblem([["package.json", "0.4.0"], ["tauri.conf.json", "0.3.9"]]);
    expect(drift).toContain("package.json = 0.4.0");
    expect(drift).toContain("tauri.conf.json = 0.3.9");
    expect(versionAlignmentProblem([["package.json", "0.4.0"], ["Cargo", undefined]])).toContain("Cargo = missing");
    expect(versionAlignmentProblem([["package.json", undefined]])).toContain("missing");
  });

  it("reads the iOS version out of the Xcode build settings", () => {
    const pbxproj = [
      "\t\t\t\tCURRENT_PROJECT_VERSION = 1;",
      "\t\t\t\tMARKETING_VERSION = 0.4.0;",
      "\t\t\t\tMARKETING_VERSION = 0.4.0;",
    ].join("\n");

    expect(xcodeMarketingVersion(pbxproj)).toBe("0.4.0");
  });

  it("treats configurations that disagree as no version at all", () => {
    // Debug and release reporting different versions is a bug, not a variant,
    // and picking one of them would hide it from the alignment check.
    const drifted = "MARKETING_VERSION = 0.4.0;\nMARKETING_VERSION = 0.3.9;";

    expect(xcodeMarketingVersion(drifted)).toBeUndefined();
    expect(xcodeMarketingVersion("")).toBeUndefined();
  });

  it("reads the Android version out of its module's build script", () => {
    const gradle = [
      "    defaultConfig {",
      '        applicationId "app.voyavpn.mobile"',
      "        versionCode 1",
      '        versionName "0.4.0"',
      "    }",
    ].join("\n");

    expect(gradleVersionName(gradle)).toBe("0.4.0");
    // A comment mentioning it is not a declaration of it.
    expect(gradleVersionName("// versionName \"0.4.0\"")).toBeUndefined();
  });
});

describe("SAFETY comment requirement", () => {
  const allowlist = [];

  it.each([
    "unsafe { ptr.read() }",
    "unsafe impl Send for Handle {}",
    'unsafe extern "C" {',
    "pub unsafe fn release(value: *mut c_char) {",
    'unsafe extern "C" fn callback(value: *mut c_char) {',
  ])("flags %s without a nearby SAFETY comment", (line) => {
    expect(findUndocumentedUnsafe(line, { allowlist }).findings).toHaveLength(1);
  });

  it("accepts a SAFETY comment within the three preceding lines", () => {
    const source = `// SAFETY: the bridge returns an owned C string.\n//\n//\nunsafe { voya_status() }`;
    expect(findUndocumentedUnsafe(source, { allowlist }).findings).toEqual([]);
  });

  it("reports the line number of each undocumented site", () => {
    const source = ["fn a() {}", "unsafe { one() }", "fn b() {}", "unsafe impl Sync for B {}"].join("\n");
    expect(findUndocumentedUnsafe(source, { allowlist }).findings.map((item) => item.line)).toEqual([2, 4]);
  });

  it("suppresses only the exact allowlisted line in the allowlisted file", () => {
    const entry = { path: "crates/voya-platform/src/tun/macos/bridge.rs", line: 'unsafe extern "C" {' };
    const source = `${entry.line}\n    unsafe { something() }\n`;

    expect(findUndocumentedUnsafe(source, { path: entry.path, allowlist: [entry] }).findings).toEqual([
      { line: 2, text: "unsafe { something() }" },
    ]);
    // The same line in another file is still a failure.
    expect(
      findUndocumentedUnsafe(entry.line, { path: "crates/voya-platform/src/other.rs", allowlist: [entry] }).findings,
    ).toHaveLength(1);
  });

  it("ships an empty allowlist so every unsafe site carries its own SAFETY comment", () => {
    expect(KNOWN_UNSAFE_WITHOUT_SAFETY_COMMENT).toEqual([]);
  });
});

describe("test module resolution", () => {
  it("resolves child modules relative to the declaring file", () => {
    expect(moduleFileCandidates("/repo/crates/voya-core/src/lib.rs", "golden")).toEqual([
      "/repo/crates/voya-core/src/golden.rs",
      "/repo/crates/voya-core/src/golden/mod.rs",
    ]);
    expect(moduleFileCandidates("/repo/crates/voya-app/src/core_flow.rs", "tests")).toEqual([
      "/repo/crates/voya-app/src/core_flow/tests.rs",
      "/repo/crates/voya-app/src/core_flow/tests/mod.rs",
    ]);
    expect(moduleFileCandidates("/repo/crates/voya-core/src/singbox/mod.rs", "tests")).toEqual([
      "/repo/crates/voya-core/src/singbox/tests.rs",
      "/repo/crates/voya-core/src/singbox/tests/mod.rs",
    ]);
  });

  it("exempts declared test modules and not merely files named tests.rs", () => {
    const sources = {
      "/repo/crates/voya-app/src/core_flow.rs": "#[cfg(test)]\nmod tests;\n",
      "/repo/crates/voya-app/src/core_flow/tests.rs": "fn helper() {}",
      "/repo/crates/voya-core/src/lib.rs": "#[cfg(test)]\npub(crate) mod golden;\n",
      "/repo/crates/voya-core/src/golden.rs": "fn fixture() {}",
      // Declared without #[cfg(test)]: production code that only looks like tests.
      "/repo/crates/voya-db/src/database.rs": "mod tests;\n",
      "/repo/crates/voya-db/src/database/tests.rs": "use reqwest::Client;",
    };

    const resolved = resolveTestModuleFiles({
      files: Object.keys(sources),
      read: (path) => sources[path],
    });

    expect([...resolved].sort()).toEqual([
      "/repo/crates/voya-app/src/core_flow/tests.rs",
      "/repo/crates/voya-core/src/golden.rs",
    ]);
  });
});
