import { describe, expect, it } from "vitest";

import { findProductionBlockersInText } from "./check-release-readiness.mjs";

describe("release readiness production blocker scan", () => {
  it("blocks GitHub URLs in production manifest URL fields", () => {
    const matches = findProductionBlockersInText(
      "dist/release/core-assets.json",
      `{
  "assets": [{
    "upstreamUrl": "https://github.com/XTLS/Xray-core/releases/download/v1.8.24/Xray-linux-64.zip",
    "url": "https://github.com/XTLS/Xray-core/releases/download/v1.8.24/Xray-linux-64.zip"
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
    "upstreamUrl": "https://github.com/XTLS/Xray-core/releases/download/v1.8.24/Xray-linux-64.zip",
    "sourceUrl": "https://github.com/XTLS/Xray-core/releases/download/v1.8.24/Xray-linux-64.zip"
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
      `fn xray_upstream_release_evidence() -> UpstreamReleaseEvidence {
  UpstreamReleaseEvidence {
    asset_templates: UpstreamAssetTemplates {
      windows_x64: Some("https://github.com/XTLS/Xray-core/releases/download/{tag}/Xray-windows-64.zip"),
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
    let manifest = r#"{"url":"https://github.com/XTLS/Xray-core/releases/download/v1.8.24/Xray-linux-64.zip"}"#;
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
