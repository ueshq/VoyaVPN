import { afterEach, describe, expect, it, vi } from "vitest";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import {
  blockerScanFiles,
  checkCoreSeedPinning,
  checkStableEnvironment,
  hasSigningInput,
  resolveTauriConfig,
  findProductionBlockersInText,
  validateStableUpdaterConfigMetadata,
} from "./readiness.mjs";

describe("release readiness production blocker scan", () => {
  it("scans the split voya-net download and subscription modules", () => {
    expect(blockerScanFiles).toEqual(
      expect.arrayContaining(["crates/voya-net/src/download.rs", "crates/voya-net/src/subscription.rs"]),
    );
  });

  it("blocks GitHub URLs in production manifest URL fields", () => {
    const matches = findProductionBlockersInText(
      "dist/release/core-assets.json",
      `{
  "assets": [{
    "upstreamUrl": "https://github.com/voyavpn/example-core/releases/download/v1.0.0/example-core-linux-x64.gz",
    "url": "https://github.com/voyavpn/example-core/releases/download/v1.0.0/example-core-linux-x64.gz"
  }]
}`,
    );

    expect(matches).toHaveLength(1);
    expect(matches[0]).toContain("dist/release/core-assets.json:4: GitHub production download URL");
    expect(matches[0]).toContain('"url":');
  });

  it("allows GitHub URLs when they are upstream or source evidence", () => {
    const matches = findProductionBlockersInText(
      "tests/fixtures/release/core-assets.json",
      `{
  "assets": [{
    "upstreamUrl": "https://github.com/voyavpn/example-core/releases/download/v1.0.0/example-core-linux-x64.gz",
    "sourceUrl": "https://github.com/voyavpn/example-core/releases/download/v1.0.0/example-core-linux-x64.gz"
  }]
}`,
    );

    expect(matches).toEqual([]);
  });

  it("blocks GitHub production download templates outside upstream evidence", () => {
    const matches = findProductionBlockersInText(
      "crates/voya-net/src/download.rs",
      `pub fn app_package() -> ReleasePackage {
  ReleasePackage {
    templates: AssetTemplates {
      windows_x64: Some("https://github.com/voyavpn/voyavpn/releases/download/{tag}/VoyaVPN-windows-x64.zip"),
    },
  }
}`,
    );

    expect(matches).toHaveLength(1);
    expect(matches[0]).toContain("GitHub production download URL");
  });

  it("allows upstream release evidence templates", () => {
    const matches = findProductionBlockersInText(
      "crates/voya-net/src/download.rs",
      `fn app_upstream_release_evidence() -> UpstreamReleaseEvidence {
  UpstreamReleaseEvidence {
    asset_templates: UpstreamAssetTemplates {
      windows_x64: Some("https://github.com/voyavpn/voyavpn/releases/download/{tag}/VoyaVPN-windows-x64.zip"),
    },
  }
}`,
    );

    expect(matches).toEqual([]);
  });

  it("allows Voya source URL constants used by strict routing bundles", () => {
    const matches = findProductionBlockersInText(
      "crates/voya-net/src/subscription.rs",
      `pub const VOYA_ROUTING_SOURCE_URL: &str =
    "https://cdn.voyavpn.test/routing/v1.json";`,
    );

    expect(matches).toEqual([]);
  });

  it("ignores defensive guard strings and Rust test fixtures", () => {
    const matches = findProductionBlockersInText(
      "crates/voya-net/src/download.rs",
      `fn ensure_production_url_allowed(url: &str) -> Result<(), ReleaseError> {
  if url.contains("voyavpn.example") || url.contains("github.com") {
    return Err(ReleaseError::ForbiddenProductionUrl(url.to_string()));
  }
  Ok(())
}

mod tests {
  #[test]
  fn rejects_github_url() {
    let manifest = r#"{"url":"https://github.com/voyavpn/example-core/releases/download/v1.0.0/example-core-linux-x64.gz"}"#;
    assert!(manifest.contains("github.com"));
  }
}`,
    );

    expect(matches).toEqual([]);
  });

  it("blocks example hosts in production URL fields", () => {
    const matches = findProductionBlockersInText(
      "docs/release/runbook.md",
      "VOYAVPN_CDN_BASE_URL=https://stable.voyavpn.example",
    );

    expect(matches).toEqual([
      "docs/release/runbook.md:1: example production URL: VOYAVPN_CDN_BASE_URL=https://stable.voyavpn.example",
    ]);
  });
});

describe("release readiness stable environment inputs", () => {
  const stableSecrets = {
    VOYAVPN_CDN_BASE_URL: "https://cdn.voyavpn.dev/stable",
    TAURI_SIGNING_PRIVATE_KEY: "signing-key",
    TAURI_SIGNING_PRIVATE_KEY_PATH: "",
    APPLE_CERTIFICATE: "certificate",
    APPLE_CERTIFICATE_PASSWORD: "certificate-password",
    APPLE_ID: "releases@voyavpn.dev",
    APPLE_PASSWORD: "apple-password",
    APPLE_TEAM_ID: "TEAM123",
    WINDOWS_CERTIFICATE_BASE64: "windows-certificate",
    WINDOWS_CERTIFICATE_PASSWORD: "windows-certificate-password",
  };

  function stubStableEnv(updatesBaseUrl) {
    for (const [name, value] of Object.entries(stableSecrets)) {
      vi.stubEnv(name, value);
    }
    vi.stubEnv("VOYAVPN_UPDATES_BASE_URL", updatesBaseUrl);
  }

  function recordingReporter() {
    const results = { pass: [], fail: [], blocker: [] };
    return {
      results,
      pass: (label, details) => results.pass.push({ label, details }),
      fail: (label, details) => results.fail.push({ label, details }),
      blocker: (label, details) => results.blocker.push({ label, details }),
    };
  }

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("fails a stable run that never supplied an updater base URL", async () => {
    stubStableEnv("");
    const reporter = recordingReporter();

    await checkStableEnvironment(reporter, { mode: "stable", cdnBaseUrl: null, updatesBaseUrl: null });

    expect(reporter.results.pass).toEqual([]);
    expect(reporter.results.fail).toHaveLength(1);
    expect(reporter.results.fail[0].details).toEqual(["missing: VOYAVPN_UPDATES_BASE_URL or --updates-base-url"]);
  });

  it("accepts a presence boolean so a workflow need not hand it the secret", () => {
    expect(hasSigningInput("APPLE_CERTIFICATE", { APPLE_CERTIFICATE: "certificate" })).toBe(true);
    expect(hasSigningInput("APPLE_CERTIFICATE", { HAS_APPLE_CERTIFICATE: "true" })).toBe(true);
    expect(hasSigningInput("APPLE_CERTIFICATE", { HAS_APPLE_CERTIFICATE: "false" })).toBe(false);
    expect(hasSigningInput("APPLE_CERTIFICATE", {})).toBe(false);
  });

  it("accepts a stable run whose updater base URL is passed on the command line", async () => {
    stubStableEnv("");
    const reporter = recordingReporter();

    await checkStableEnvironment(reporter, {
      mode: "stable",
      cdnBaseUrl: null,
      updatesBaseUrl: "https://updates.voyavpn.dev/stable",
    });

    expect(reporter.results.fail).toEqual([]);
    expect(reporter.results.pass).toHaveLength(1);
  });
});

describe("release readiness packaged updater config evidence", () => {
  it("rejects package-time updater overlay metadata with a different approved public key", () => {
    const publicKey = "A".repeat(64);
    const metadata = {
      sha256: "1".repeat(64),
      pubkeySha256: createHash("sha256").update("B".repeat(64)).digest("hex"),
      endpoints: ["https://updates.voyavpn.dev/stable/latest.json"],
      createUpdaterArtifacts: true,
      path: "stable-updater-config.json",
    };

    expect(() =>
      validateStableUpdaterConfigMetadata(metadata, {
        updatesBaseUrl: "https://updates.voyavpn.dev/stable",
        updaterPublicKey: publicKey,
        label: "artifact-manifest.json",
      }),
    ).toThrow(/public key hash/);
  });
});

describe("release readiness core seed pinning", () => {
  function recordingReporter(mode = "dry-run") {
    const results = { pass: [], fail: [], warn: [] };
    return {
      results,
      pass: (label, details) => results.pass.push({ label, details }),
      fail: (label, details) => results.fail.push({ label, details }),
      warn: (label, details) => results.warn.push({ label, details }),
      blocker: (label, details) =>
        mode === "stable" ? results.fail.push({ label, details }) : results.warn.push({ label, details }),
    };
  }

  it("passes when no seed is staged and when the staged seed matches the pin", async () => {
    const absent = recordingReporter();
    await checkCoreSeedPinning(absent, { verifySeed: () => ({ ok: false, staged: false }) });
    expect(absent.results.fail).toEqual([]);
    expect(absent.results.pass).toHaveLength(1);

    const verified = recordingReporter();
    await checkCoreSeedPinning(verified, {
      verifySeed: () => ({
        manifest: { assetName: "sing-box-1.13.14-linux-amd64.tar.gz", sha256: "a".repeat(64) },
        ok: true,
        pinned: true,
        staged: true,
      }),
    });
    expect(verified.results.fail).toEqual([]);
    expect(verified.results.pass[0].details[0]).toContain("matches the pinned SHA-256");
  });

  it("fails outright on a digest mismatch and blocks a stale or unpinned seed", async () => {
    const tampered = recordingReporter();
    await checkCoreSeedPinning(tampered, {
      verifySeed: () => ({ code: "executable-digest-mismatch", ok: false, reason: "digest differs", staged: true }),
    });
    expect(tampered.results.fail).toHaveLength(1);

    const stale = recordingReporter("dry-run");
    await checkCoreSeedPinning(stale, {
      verifySeed: () => ({ code: "version-mismatch", ok: false, reason: "staged seed is v1.0.0", staged: true }),
    });
    expect(stale.results.fail).toEqual([]);
    expect(stale.results.warn).toHaveLength(1);

    const unpinned = recordingReporter("stable");
    await checkCoreSeedPinning(unpinned, {
      verifySeed: () => ({
        manifest: { assetName: "sing-box-9.9.9-linux-amd64.tar.gz" },
        ok: true,
        pinned: false,
        staged: true,
      }),
    });
    expect(unpinned.results.fail).toHaveLength(1);
    expect(unpinned.results.fail[0].details[0]).toContain("VOYAVPN_ALLOW_UNPINNED_SING_BOX");
  });
});

describe("release readiness dry-run gate", () => {
  const execFileAsync = promisify(execFile);
  const repoRoot = resolve(fileURLToPath(new URL("../../..", import.meta.url)));
  const workDirs = [];

  afterEach(async () => {
    await Promise.all(workDirs.splice(0).map((path) => rm(path, { force: true, recursive: true })));
  });

  it("runs the whole dry-run gate end to end and reports PASS", async () => {
    const workDir = await mkdtemp(join(tmpdir(), "voyavpn-readiness-dry-run-"));
    workDirs.push(workDir);

    const { stdout } = await execFileAsync(
      process.execPath,
      ["scripts/release/cli.mjs", "readiness", "--mode", "dry-run", "--work-dir", workDir],
      { cwd: repoRoot },
    );

    expect(stdout).toContain("Readiness result: PASS");
    expect(stdout).toContain("[PASS] required release documents");
    expect(stdout).toContain("[PASS] stable release index manifest");
    expect(stdout).toContain("[PASS] stable updater metadata");
    expect(stdout).toContain("[PASS] stable core asset manifest");
    expect(stdout).not.toContain("[FAIL]");
  }, 60_000);

  it("defaults stable to the generated updater overlay and says how to make it", () => {
    const stable = { mode: "stable", tauriConfig: null };

    expect(resolveTauriConfig(stable, { configExists: () => true })).toBe(
      "target/release-config/tauri.updater.stable.generated.json",
    );
    expect(() => resolveTauriConfig(stable, { configExists: () => false })).toThrow(
      /Run `pnpm release -- updater-config` first/,
    );

    // The credential-free committed config is only ever scanned by dry runs.
    expect(resolveTauriConfig({ mode: "dry-run", tauriConfig: null }, { configExists: () => false })).toBe(
      "apps/desktop/src-tauri/tauri.conf.json",
    );
    expect(resolveTauriConfig({ mode: "stable", tauriConfig: "custom.json" }, { configExists: () => false })).toBe(
      "custom.json",
    );
  });
});
