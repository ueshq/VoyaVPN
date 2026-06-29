import { describe, expect, it } from "vitest";
import { createHash } from "node:crypto";

import { findProductionBlockersInText, validateStableUpdaterConfigMetadata } from "./check-release-readiness.mjs";

describe("release readiness production blocker scan", () => {
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
      "crates/voya-net/src/update.rs",
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
      "crates/voya-net/src/update.rs",
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

  it("allows source URL constants that preserve upstream acquisition evidence", () => {
    const matches = findProductionBlockersInText(
      "crates/voya-net/src/lib.rs",
      `pub const RUSSIA_GEO_SOURCE_URL: &str =
    "https://github.com/runetfreedom/russia-v2ray-rules-dat/releases/latest/download/{0}.dat";`,
    );

    expect(matches).toEqual([]);
  });

  it("ignores defensive guard strings and Rust test fixtures", () => {
    const matches = findProductionBlockersInText(
      "crates/voya-net/src/update.rs",
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
