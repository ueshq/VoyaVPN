use serde_json::{json, Value};
use tempfile::TempDir;
use voya_platform::tun::TunBackend;

use super::*;

fn request(config_path: &Path) -> NativeTunStartRequest {
    NativeTunStartRequest {
        backend: TunBackend::IosPacketTunnel,
        active_profile_id: Some("profile-1".to_string()),
        kill_switch: true,
        main_config_path: config_path.to_path_buf(),
        pre_config_path: None,
    }
}

fn write_config(dir: &Path, config: &Value) -> PathBuf {
    let path = dir.join("config.json");
    fs::write(&path, serde_json::to_string(config).expect("serializes")).expect("writes");
    path
}

#[test]
fn the_payload_carries_the_config_inline_with_the_provider_paths() {
    let temp = TempDir::new().expect("temp dir");
    let shared = temp.path().join("shared");
    let config = json!({ "log": { "level": "warn" }, "outbounds": [] });
    let config_path = write_config(temp.path(), &config);

    let handoff: Value =
        serde_json::from_str(&build_handoff(&request(&config_path), &shared).expect("builds"))
            .expect("valid JSON");

    assert_eq!(handoff["version"], 1);
    assert_eq!(handoff["activeProfileId"], "profile-1");
    assert_eq!(handoff["mainConfigPath"], config_path.display().to_string());
    assert_eq!(
        handoff["statusPath"],
        shared.join("PT/status.json").display().to_string()
    );
    assert_eq!(
        handoff["logPath"],
        shared.join("PT/provider.log").display().to_string()
    );
    // The config travels as text, not as an object: the provider hands it to
    // Libbox verbatim and re-encoding it here is one more chance to change it.
    let inlined: Value =
        serde_json::from_str(handoff["singboxConfigJson"].as_str().expect("text")).expect("JSON");
    assert_eq!(inlined, config);
}

#[test]
fn a_config_with_nothing_to_stage_is_passed_through_byte_for_byte() {
    let temp = TempDir::new().expect("temp dir");
    let shared = temp.path().join("shared");
    // Remote rule sets and relative local ones both stay as they are: a
    // relative path is resolved against the provider's own working directory.
    let config = json!({
        "route": {
            "rule_set": [
                { "type": "remote", "url": "https://example.test/geoip.srs" },
                { "type": "local", "path": "relative/geosite.srs" },
            ]
        }
    });
    let config_path = write_config(temp.path(), &config);
    let original = fs::read_to_string(&config_path).expect("reads");

    let handoff: Value =
        serde_json::from_str(&build_handoff(&request(&config_path), &shared).expect("builds"))
            .expect("valid JSON");

    assert_eq!(handoff["singboxConfigJson"], original);
    assert!(!shared.join("srss").exists(), "nothing should be staged");
}

#[test]
fn a_local_rule_set_is_copied_into_the_shared_container_and_repointed() {
    let temp = TempDir::new().expect("temp dir");
    let shared = temp.path().join("shared");
    let rule_set = temp.path().join("geosite-cn.srs");
    fs::write(&rule_set, b"rule-set bytes").expect("writes");
    let config = json!({
        "route": {
            "rule_set": [
                { "type": "local", "path": rule_set.display().to_string(), "tag": "geosite-cn" },
                { "type": "remote", "url": "https://example.test/geoip.srs" },
            ]
        }
    });
    let config_path = write_config(temp.path(), &config);

    let handoff: Value =
        serde_json::from_str(&build_handoff(&request(&config_path), &shared).expect("builds"))
            .expect("valid JSON");

    let staged = shared.join("srss/geosite-cn.srs");
    assert_eq!(fs::read(&staged).expect("staged file"), b"rule-set bytes");

    let inlined: Value =
        serde_json::from_str(handoff["singboxConfigJson"].as_str().expect("text")).expect("JSON");
    let entries = inlined["route"]["rule_set"].as_array().expect("array");
    assert_eq!(entries[0]["path"], staged.display().to_string());
    // Everything else about the entry survives the rewrite.
    assert_eq!(entries[0]["tag"], "geosite-cn");
    assert_eq!(entries[1]["url"], "https://example.test/geoip.srs");
}

#[test]
fn a_rule_set_that_cannot_be_staged_fails_the_start_rather_than_shipping_a_dead_path() {
    let temp = TempDir::new().expect("temp dir");
    let shared = temp.path().join("shared");
    let config = json!({
        "route": {
            "rule_set": [
                { "type": "local", "path": temp.path().join("absent.srs").display().to_string() },
            ]
        }
    });
    let config_path = write_config(temp.path(), &config);

    let error = build_handoff(&request(&config_path), &shared).expect_err("no such rule set");

    assert!(
        matches!(error, HandoffError::StageRuleSet { .. }),
        "unexpected error: {error}"
    );
}

#[test]
fn an_unreadable_or_malformed_config_is_named_in_the_failure() {
    let temp = TempDir::new().expect("temp dir");
    let shared = temp.path().join("shared");
    let missing = temp.path().join("gone.json");

    assert!(matches!(
        build_handoff(&request(&missing), &shared).expect_err("no config"),
        HandoffError::ReadConfig { .. }
    ));

    let malformed = temp.path().join("config.json");
    fs::write(&malformed, "not json").expect("writes");
    assert!(matches!(
        build_handoff(&request(&malformed), &shared).expect_err("bad config"),
        HandoffError::InvalidConfig { .. }
    ));
}
