//! What the packaged probe core was built with.
//!
//! The Mac App Store package ships a sing-box seed compiled from source
//! without `with_naive_outbound` (Cronet imports a non-public CoreFoundation
//! symbol App Review rejects). sing-box refuses a whole config that names an
//! outbound type it was not built with, so the speed test reads the seed's
//! build tags from `sing-box.seed.json` and leaves such nodes out of a page
//! rather than failing every node beside them.

use std::{collections::BTreeSet, path::Path};

use voya_core::ConfigType;
use voya_platform::{coreinfo::CORE_DIR_NAME, filesystem, paths::core_seed_resource_dir};

/// The manifest the build scripts write beside the seed executable.
const SEED_MANIFEST: &str = "sing-box.seed.json";

/// The build tags of a probe core, when they are known.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProbeCoreCapabilities {
    build_tags: Option<BTreeSet<String>>,
}

impl ProbeCoreCapabilities {
    #[must_use]
    pub fn from_build_tags<I, S>(tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            build_tags: Some(tags.into_iter().map(Into::into).collect()),
        }
    }

    /// Reads `tags` from a seed manifest. The upstream archive's manifest has
    /// none, and an unreadable one says nothing either; both mean unknown,
    /// which admits every outbound type as before.
    #[must_use]
    pub fn from_seed_manifest_json(bytes: &[u8]) -> Self {
        let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            return Self::default();
        };
        let Some(tags) = manifest.get("tags").and_then(serde_json::Value::as_array) else {
            return Self::default();
        };
        Self::from_build_tags(tags.iter().filter_map(serde_json::Value::as_str))
    }

    /// The capabilities of the seed under a packaged `core-seeds` directory.
    pub(super) fn read_from_seed_resources(seed_resources_dir: &Path) -> Self {
        let path = core_seed_resource_dir(seed_resources_dir, CORE_DIR_NAME).join(SEED_MANIFEST);
        match filesystem::read_file_if_exists(&path) {
            Ok(Some(bytes)) => Self::from_seed_manifest_json(&bytes),
            Ok(None) => Self::default(),
            Err(error) => {
                tracing::warn!(?error, "failed to read the sing-box seed manifest");
                Self::default()
            }
        }
    }

    /// The build tag `config_type` needs and this core lacks, if any.
    #[must_use]
    pub fn missing_build_tag(&self, config_type: ConfigType) -> Option<&'static str> {
        let tags = self.build_tags.as_ref()?;
        config_type
            .required_build_tag()
            .filter(|tag| !tags.contains(*tag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_seed_without_the_naive_tag_refuses_only_naive() {
        let capabilities = ProbeCoreCapabilities::from_seed_manifest_json(
            br#"{"origin":"source","tags":["with_quic","with_wireguard","with_clash_api"]}"#,
        );
        assert_eq!(
            capabilities.missing_build_tag(ConfigType::Naive),
            Some("with_naive_outbound")
        );
        for supported in [
            ConfigType::Shadowsocks,
            ConfigType::VLESS,
            ConfigType::Hysteria2,
            ConfigType::WireGuard,
        ] {
            assert_eq!(capabilities.missing_build_tag(supported), None);
        }
    }

    #[test]
    fn unknown_tags_admit_every_outbound_type() {
        for manifest in [
            br#"{"assetName":"sing-box-1.13.14-darwin-arm64.tar.gz"}"#.as_slice(),
            b"not json",
            br#"{"tags":"with_quic"}"#,
        ] {
            let capabilities = ProbeCoreCapabilities::from_seed_manifest_json(manifest);
            assert_eq!(capabilities, ProbeCoreCapabilities::default());
            assert_eq!(capabilities.missing_build_tag(ConfigType::Naive), None);
        }
    }

    #[test]
    fn reads_the_manifest_beside_the_packaged_seed() {
        let root = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            ProbeCoreCapabilities::read_from_seed_resources(root.path()),
            ProbeCoreCapabilities::default()
        );
        let seed = root.path().join(CORE_DIR_NAME);
        filesystem::write_file_with_parent(&seed.join(SEED_MANIFEST), br#"{"tags":["with_quic"]}"#)
            .expect("write manifest");
        assert_eq!(
            ProbeCoreCapabilities::read_from_seed_resources(root.path()),
            ProbeCoreCapabilities::from_build_tags(["with_quic"])
        );
    }
}
